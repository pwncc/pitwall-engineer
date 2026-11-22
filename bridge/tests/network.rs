use base64::Engine;
use lmu_pitwall::engineer::{
    model::*,
    server::{self, App, Credential},
    transport,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, timeout, Duration},
};
fn port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
async fn connect(c: &AgentConfig) -> anyhow::Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let tcp = TcpStream::connect(&c.endpoint).await?;
    Ok(
        tokio_rustls::TlsConnector::from(transport::client_config(c)?)
            .connect(
                tokio_rustls::rustls::pki_types::ServerName::try_from("lmu-engineer")?.to_owned(),
                tcp,
            )
            .await?,
    )
}
async fn http(port: u16, path: &str, host: &str, origin: Option<&str>) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let o = origin
        .map(|s| format!("Origin: {s}\r\n"))
        .unwrap_or_default();
    s.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n{o}Connection: close\r\n\r\n").as_bytes(),
    )
    .await
    .unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    String::from_utf8_lossy(&out).into()
}
async fn source_online(port: u16, source: &str) -> bool {
    let response = http(port, "/api/state", &format!("127.0.0.1:{port}"), None).await;
    let body: serde_json::Value =
        serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
    body["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["source_id"] == source)
        .unwrap()["online"]
        .as_bool()
        .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tls_credentials_multicar_reconnect_revocation_and_http_origin() {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into()).unwrap();
    let hp = port();
    let ip = port();
    let mut configs = vec![];
    {
        let mut a = app.lock().unwrap();
        for n in 0..8 {
            let source_id = new_id();
            let token = new_id();
            let c = AgentConfig {
                endpoint: format!("127.0.0.1:{ip}"),
                certificate_der: base64::engine::general_purpose::STANDARD.encode(&a.cert),
                token: token.clone(),
                source_id: source_id.clone(),
                event_id: a.event_id.clone(),
                entry_id: format!("Car {n}"),
                label: format!("PC {n}"),
                legacy: false,
                rest_supplement: false,
                ..Default::default()
            };
            let credential = Credential {
                source_id,
                token,
                event_id: c.event_id.clone(),
                entry_id: c.entry_id.clone(),
                label: c.label.clone(),
                revoked: false,
                ..Default::default()
            };
            a.credentials.push(credential);
            configs.push(c);
        }
    }
    let task = tokio::spawn(server::run(app.clone(), hp, ip, true));
    sleep(Duration::from_millis(100)).await;
    assert!(http(hp, "/api/state", &format!("127.0.0.1:{hp}"), None)
        .await
        .starts_with("HTTP/1.1 200"));
    assert!(http(hp, "/api/state", "evil.example", None)
        .await
        .starts_with("HTTP/1.1 403"));
    assert!(http(
        hp,
        "/api/state",
        &format!("localhost:{hp}"),
        Some("https://evil.example")
    )
    .await
    .starts_with("HTTP/1.1 403"));
    let mut bad = configs[0].clone();
    bad.token = "wrong".into();
    let mut conn = connect(&bad).await.unwrap();
    transport::send(
        &mut conn,
        &Hello {
            version: 1,
            source_id: bad.source_id.clone(),
            token: bad.token,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(transport::receive::<Ack, _>(&mut conn).await.is_err());
    assert!(app.lock().unwrap().source_seen.is_empty());
    let other = tempfile::tempdir().unwrap();
    let (cert, _) = transport::certificate(other.path()).unwrap();
    bad = configs[0].clone();
    bad.certificate_der = base64::engine::general_purpose::STANDARD.encode(cert);
    assert!(connect(&bad).await.is_err());
    for c in &configs {
        let mut conn = connect(c).await.unwrap();
        transport::send(
            &mut conn,
            &Hello {
                version: 1,
                source_id: c.source_id.clone(),
                token: c.token.clone(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(
            transport::receive::<Ack, _>(&mut conn)
                .await
                .unwrap()
                .accepted
        );
        assert!(source_online(hp, &c.source_id).await);
        app.lock()
            .unwrap()
            .source_seen
            .insert(c.source_id.clone(), now_ms() - 21000);
        assert!(!source_online(hp, &c.source_id).await);
        let batch = Batch {
            version: 1,
            event_id: c.event_id.clone(),
            entry_id: c.entry_id.clone(),
            source_id: c.source_id.clone(),
            run_id: "test-run".into(),
            seq: 1,
            historical: false,
            samples: vec![Sample {
                captured_ms: now_ms(),
                driver: c.label.clone(),
                authoritative: true,
                ..Default::default()
            }],
        };
        for _ in 0..2 {
            transport::send(&mut conn, &batch).await.unwrap();
            assert!(
                transport::receive::<Ack, _>(&mut conn)
                    .await
                    .unwrap()
                    .accepted
            );
        }
        let received = app.lock().unwrap().entries[&c.entry_id].received;
        let heartbeat = Batch {
            version: PROTOCOL,
            event_id: c.event_id.clone(),
            entry_id: c.entry_id.clone(),
            source_id: c.source_id.clone(),
            run_id: "heartbeat".into(),
            seq: 0,
            historical: false,
            samples: vec![],
        };
        transport::send(&mut conn, &heartbeat).await.unwrap();
        let ack: Ack = transport::receive(&mut conn).await.unwrap();
        assert!(ack.accepted && ack.seq == 0);
        assert!(source_online(hp, &c.source_id).await);
        assert_eq!(app.lock().unwrap().entries[&c.entry_id].received, received);
    }
    assert_eq!(app.lock().unwrap().entries.len(), 8);
    assert_eq!(app.lock().unwrap().live_batches, 8);
    let c = &configs[0];
    let mut conn = connect(c).await.unwrap();
    transport::send(
        &mut conn,
        &Hello {
            version: 1,
            source_id: c.source_id.clone(),
            token: c.token.clone(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        transport::receive::<Ack, _>(&mut conn)
            .await
            .unwrap()
            .accepted
    );
    app.lock().unwrap().credentials[0].revoked = true;
    let batch = Batch {
        version: 1,
        event_id: c.event_id.clone(),
        entry_id: c.entry_id.clone(),
        source_id: c.source_id.clone(),
        run_id: "test-run".into(),
        seq: 2,
        historical: false,
        samples: vec![Sample::default()],
    };
    transport::send(&mut conn, &batch).await.unwrap();
    assert!(timeout(
        Duration::from_secs(2),
        transport::receive::<Ack, _>(&mut conn)
    )
    .await
    .unwrap()
    .is_err());
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paired_pcs_handover_preserves_entry_and_ignores_old_seat() {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into()).unwrap();
    let hp = port();
    let ip = port();
    let mut configs = Vec::new();
    {
        let mut a = app.lock().unwrap();
        for label in ["Alice PC", "Bob PC"] {
            let c = AgentConfig {
                endpoint: format!("127.0.0.1:{ip}"),
                certificate_der: base64::engine::general_purpose::STANDARD.encode(&a.cert),
                token: new_id(),
                source_id: new_id(),
                event_id: a.event_id.clone(),
                entry_id: "Car 27".into(),
                label: label.into(),
                legacy: false,
                rest_supplement: false,
                ..Default::default()
            };
            a.credentials.push(Credential {
                source_id: c.source_id.clone(),
                token: c.token.clone(),
                event_id: c.event_id.clone(),
                entry_id: c.entry_id.clone(),
                label: c.label.clone(),
                revoked: false,
                ..Default::default()
            });
            configs.push(c);
        }
    }
    let task = tokio::spawn(server::run(app.clone(), hp, ip, true));
    sleep(Duration::from_millis(100)).await;
    let mut connections = Vec::new();
    for c in &configs {
        let mut conn = connect(c).await.unwrap();
        transport::send(
            &mut conn,
            &Hello {
                version: PROTOCOL,
                token: c.token.clone(),
                source_id: c.source_id.clone(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let ack: Ack = transport::receive(&mut conn).await.unwrap();
        assert!(ack.accepted);
        connections.push(conn);
    }
    let captured = now_ms();
    // Separate authenticated connections represent two paired PCs. Bob first
    // watches, then takes the seat; Alice's remaining packets arrive afterwards.
    for (seq, pc, driver, game, authoritative, fuel) in [
        (1, 0, "Alice", 100., true, Some(60.)),
        (2, 0, "Alice", 101., true, Some(59.9)),
        (3, 1, "Bob", 101.1, false, None),
        (4, 1, "Bob", 102., true, Some(59.8)),
        (5, 0, "Alice", 101.5, true, Some(59.85)),
        (6, 0, "Alice", 103., false, None),
        (7, 1, "Bob", 103., true, Some(59.7)),
    ] {
        let c = &configs[pc];
        let b = Batch {
            version: PROTOCOL,
            event_id: c.event_id.clone(),
            entry_id: c.entry_id.clone(),
            source_id: c.source_id.clone(),
            run_id: c.source_id.clone(),
            seq,
            historical: false,
            samples: vec![Sample {
                captured_ms: captured + seq * 20,
                game_s: game,
                driver: driver.into(),
                vehicle: "Shared LMH".into(),
                track: "Bahrain".into(),
                track_length_m: 5412.,
                lap: 1,
                lap_distance_m: 200.,
                slot: Some(27),
                session_kind: 10,
                authoritative,
                valid_lap: true,
                fuel_l: fuel,
                fuel_capacity_l: Some(100.),
                ..Default::default()
            }],
        };
        transport::send(&mut connections[pc], &b).await.unwrap();
        let ack: Ack = transport::receive(&mut connections[pc]).await.unwrap();
        assert!(ack.accepted, "{:?}", ack.error);
        if seq >= 4 {
            let a = app.lock().unwrap();
            assert_eq!(a.entries.len(), 1);
            let e = &a.entries["Car 27"];
            assert_eq!(e.last.driver, "Bob");
            assert!(e.last.authoritative);
            assert_eq!(e.source, configs[1].source_id);
            assert!(e.last.fuel_l.unwrap() < 60.);
            assert_eq!(e.stint_start, 102.);
        }
    }
    {
        let a = app.lock().unwrap();
        let e = &a.entries["Car 27"];
        assert_eq!(e.drive_times["Alice"], 1.);
        assert_eq!(e.drive_times["Bob"], 1.);
        let changes: Vec<_> = a
            .storage
            .events()
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == "driver")
            .collect();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text, "Alice → Bob");
        assert!(a.errors.is_empty());
    }
    drop(connections);
    task.abort();
    let restored = App::open(dir.path().into()).unwrap();
    let a = restored.lock().unwrap();
    let e = &a.entries["Car 27"];
    assert_eq!(e.last.driver, "Bob");
    assert_eq!(e.received, 0);
    assert_eq!(e.drive_times["Alice"], 1.);
    assert_eq!(e.drive_times["Bob"], 1.);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn historical_backfill_repairs_laps_without_replacing_live_state() {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into()).unwrap();
    let event = app.lock().unwrap().event_id.clone();
    let end = now_ms();
    let sample = |tick: u64| {
        let t = tick as f64 / 10.;
        Sample {
            captured_ms: end - 240000 + tick * 100,
            game_s: t,
            driver: "Alice".into(),
            vehicle: "GT3".into(),
            track: "Monza".into(),
            track_length_m: 5793.,
            lap: (t / 120.) as i32,
            lap_distance_m: t % 120. / 120. * 5793.,
            lap_start_s: (t / 120.).floor() * 120.,
            last_lap_s: Some(120.),
            authoritative: true,
            valid_lap: true,
            fuel_l: Some(100. - t / 40.),
            fuel_capacity_l: Some(100.),
            in_pits: (130. ..131.).contains(&t),
            ..Default::default()
        }
    };
    let batch = |seq: u64, historical: bool, samples: Vec<Sample>| Batch {
        version: 1,
        event_id: event.clone(),
        entry_id: "Backfill car".into(),
        source_id: "source".into(),
        run_id: "run".into(),
        seq,
        historical,
        samples,
    };
    app.lock()
        .unwrap()
        .ingest(&batch(9999, false, vec![sample(2400)]))
        .unwrap();
    for block in (0..12).rev() {
        let samples = (block * 200..(block + 1) * 200).map(&sample).collect();
        app.lock()
            .unwrap()
            .ingest(&batch(block + 1, true, samples))
            .unwrap();
    }
    assert_eq!(
        app.lock().unwrap().entries["Backfill car"].last.game_s,
        240.
    );
    assert!(app.lock().unwrap().entries["Backfill car"].laps.is_empty());
    let task = tokio::spawn(server::run(app.clone(), port(), port(), true));
    timeout(Duration::from_secs(12), async {
        loop {
            if app.lock().unwrap().entries["Backfill car"].laps.len() == 2 {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("backfill reconciliation did not complete");
    {
        let a = app.lock().unwrap();
        let e = &a.entries["Backfill car"];
        assert_eq!(e.last.game_s, 240.);
        assert!(e.drive_times["Alice"] > 239.);
        assert_eq!(e.laps.len(), 2);
        assert_eq!(
            a.storage
                .lap_metadata(&event, "Backfill car")
                .unwrap()
                .len(),
            2
        );
        assert!(a.errors.is_empty(), "{:?}", a.errors);
    }
    let before = app.lock().unwrap().storage.events().unwrap().len();
    let restored = App::open(dir.path().into()).unwrap();
    let restored = restored.lock().unwrap();
    assert_eq!(restored.entries["Backfill car"].received, 0);
    assert_eq!(restored.storage.events().unwrap().len(), before);
    task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn receiver_stop_signal_closes_http_listener() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into()).unwrap();
    let hp = port();
    let ip = port();
    let stop = Arc::new(AtomicBool::new(false));
    let task = tokio::spawn(server::run_with_stop(app, hp, ip, true, stop.clone()));
    sleep(Duration::from_millis(150)).await;
    assert!(http(hp, "/api/state", &format!("127.0.0.1:{hp}"), None)
        .await
        .contains("200 OK"));
    stop.store(true, Ordering::Relaxed);
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(TcpStream::connect(("127.0.0.1", hp)).await.is_err());
}
