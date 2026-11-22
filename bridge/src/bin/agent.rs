#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use anyhow::{ensure, Context, Result};
use clap::Parser;
use lmu_pitwall::engineer::{capture::Capture, model::*, transport};
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::TcpStream, time::timeout};
#[derive(Parser)]
#[command(name = "Pitwall Engineer driver agent", version = "0.1.1")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    demo: bool,
    #[arg(long)]
    no_tray: bool,
    #[arg(long, default_value_t = 50)]
    hz: u32,
    #[arg(long)]
    seconds: Option<u64>,
    #[arg(long, default_value_t = 1.)]
    speed: f64,
}
fn configuration(path: Option<PathBuf>) -> Result<AgentConfig> {
    if let Some(p) = path {
        return Ok(serde_json::from_slice(&std::fs::read(p)?)?);
    }
    let exe = std::env::current_exe()?;
    let mut f = std::fs::File::open(&exe)?;
    if f.metadata()?.len() > 12 {
        f.seek(SeekFrom::End(-12))?;
        let mut footer = [0u8; 12];
        f.read_exact(&mut footer)?;
        if &footer[4..] == b"LMUENG01" {
            let len = u32::from_le_bytes(footer[..4].try_into()?) as usize;
            ensure!(len <= 16384, "embedded configuration too large");
            f.seek(SeekFrom::End(-12 - len as i64))?;
            let mut b = vec![0; len];
            f.read_exact(&mut b)?;
            return Ok(serde_json::from_slice(&b)?);
        }
    }
    Ok(serde_json::from_slice(&std::fs::read(exe.with_file_name("agent.json")).context("Use a paired driver package exported by LMU Engineer, or place agent.json beside this EXE")?)?)
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let path = std::env::temp_dir().join("lmu-agent-error.txt");
        let _ = std::fs::write(
            &path,
            format!("LMU telemetry agent could not start:\r\n{e:#}"),
        );
        let _ = open::that(path);
    }
}
async fn run() -> Result<()> {
    let args = Args::parse();
    let local_root = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()))
        .join("LMUEngineerAgent");
    let c = lmu_pitwall::engineer::identity::resolve(configuration(args.config)?, &local_root)?;
    ensure!(
        c.source_id.len() == 32 && c.source_id.chars().all(|c| c.is_ascii_hexdigit()),
        "Invalid source identifier in pairing configuration"
    );
    let root = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()))
        .join("LMUEngineerAgent")
        .join(&c.source_id);
    std::fs::create_dir_all(root.join("spool"))?;
    let singleton = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("agent.lock"))?;
    // OS file locking releases automatically on process exit, including crashes.
    match singleton.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let file = tracing_appender::rolling::daily(root.join("logs"), "agent.log");
    let (writer, _guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::fmt().with_writer(writer).init();
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let tls = transport::client_config(&c)?;
    let connector = tokio_rustls::TlsConnector::from(tls);
    let recovered = lmu_pitwall::engineer::spool::recover(&root, &c)?;
    tracing::info!("Recovered {recovered} interrupted batches");
    let stop = Arc::new(AtomicBool::new(false));
    if !args.no_tray {
        lmu_pitwall::engineer::tray::start(
            root.clone(),
            stop.clone(),
            format!("LMU collector · {}", c.label),
        );
    }
    let finished = Arc::new(AtomicBool::new(false));
    let capture_finished = finished.clone();
    let capture_stop = stop.clone();
    let capture_root = root.clone();
    let config = c.clone();
    let hz = args.hz.clamp(5, 100);
    let demo = args.demo;
    let seconds = args.seconds;
    let speed = args.speed.clamp(1., 1000.);
    tokio::task::spawn_blocking(move || {
        if let Err(e) = collect(
            config,
            &capture_root,
            hz,
            demo,
            seconds,
            speed,
            capture_stop,
        ) {
            tracing::error!("Capture stopped: {e:#}");
            write_status(&capture_root, "Capture stopped", &e.to_string());
        }
        capture_finished.store(true, Ordering::Release);
    });
    let capture_stop = stop.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        capture_stop.store(true, Ordering::Relaxed);
    });
    let mut delay = 1;
    while !stop.load(Ordering::Relaxed) {
        let result: Result<()> = async {
            let tcp = timeout(Duration::from_secs(10), TcpStream::connect(&c.endpoint)).await??;
            tcp.set_nodelay(true)?;
            let name =
                tokio_rustls::rustls::pki_types::ServerName::try_from("lmu-engineer")?.to_owned();
            let mut conn = timeout(Duration::from_secs(10), connector.connect(name, tcp)).await??;
            transport::send(
                &mut conn,
                &Hello {
                    version: PROTOCOL,
                    token: c.token.clone(),
                    source_id: c.source_id.clone(),
                    enrollment_id: c.enrollment_id.clone(),
                    device_name: c.enrollment_id.as_ref().map(|_| {
                        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Driver PC".into())
                    }),
                },
            )
            .await?;
            let ack: Ack =
                timeout(Duration::from_secs(10), transport::receive(&mut conn)).await??;
            ensure!(ack.accepted, "receiver rejected pairing");
            delay = 1;
            let mut heartbeat = std::time::Instant::now();
            while !stop.load(Ordering::Relaxed) {
                // Observe completion before enumerating files: the writer may
                // finish a final rename while an earlier enumeration is empty.
                let capture_done = finished.load(Ordering::Acquire);
                let mut files: Vec<_> = std::fs::read_dir(root.join("spool"))?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|s| s == "batch"))
                    .map(|e| e.path())
                    .collect();
                files.sort();
                if files.is_empty() {
                    if capture_done {
                        stop.store(true, Ordering::Relaxed);
                        break;
                    }
                    if heartbeat.elapsed() >= Duration::from_secs(10) {
                        transport::send(
                            &mut conn,
                            &Batch {
                                version: PROTOCOL,
                                event_id: c.event_id.clone(),
                                entry_id: c.entry_id.clone(),
                                source_id: c.source_id.clone(),
                                run_id: "heartbeat".into(),
                                seq: 0,
                                historical: false,
                                samples: vec![],
                            },
                        )
                        .await?;
                        let ack: Ack =
                            timeout(Duration::from_secs(10), transport::receive(&mut conn))
                                .await??;
                        ensure!(ack.seq == 0 && ack.accepted, "Receiver rejected heartbeat");
                        heartbeat = std::time::Instant::now();
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
                // Prioritize fresh data. Historic backfill never overwrites the receiver's live state.
                let newest = files.last().unwrap();
                let fresh = newest
                    .file_name()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.split('-').next())
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_some_and(|t| now_ms().saturating_sub(t) < 3000);
                let path = if fresh { newest } else { &files[0] };
                let raw = std::fs::read(path)?;
                let mut b: Batch = rmp_serde::from_slice(&raw)?;
                b.historical = !fresh;
                transport::send(&mut conn, &b).await?;
                let ack: Ack =
                    timeout(Duration::from_secs(20), transport::receive(&mut conn)).await??;
                ensure!(
                    ack.seq == b.seq && ack.accepted,
                    "batch rejected: {:?}",
                    ack.error
                );
                heartbeat = std::time::Instant::now();
                std::fs::remove_file(path)?;
                write_status(
                    &root,
                    "Connected",
                    &format!(
                        "{} · {} queued batches",
                        c.label,
                        files.len().saturating_sub(1)
                    ),
                );
                if !fresh {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
            let _ = timeout(
                Duration::from_secs(2),
                tokio::io::AsyncWriteExt::shutdown(&mut conn),
            )
            .await;
            Ok(())
        }
        .await;
        if let Err(e) = result {
            tracing::warn!("Link: {e:#}");
            write_status(&root, "Reconnecting", &e.to_string());
            tokio::time::sleep(Duration::from_secs(delay)).await;
            delay = (delay * 2).min(30);
        }
    }
    Ok(())
}
fn write_status(root: &Path, state: &str, detail: &str) {
    let _ = std::fs::write(
        root.join("status.json"),
        serde_json::to_vec(&serde_json::json!({"at_ms":now_ms(),"state":state,"detail":detail}))
            .unwrap_or_default(),
    );
}
fn collect(
    c: AgentConfig,
    root: &Path,
    hz: u32,
    demo: bool,
    seconds: Option<u64>,
    speed: f64,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let mut capture = Capture::new();
    let mut legacy = lmu_pitwall::engineer::legacy::LegacyCapture::new();
    let supplement = lmu_pitwall::engineer::supplement::Supplement::new();
    let run = new_id();
    let mut seq = 0u64;
    let mut samples = Vec::with_capacity(hz as usize);
    let start = std::time::Instant::now();
    let mut tick = std::time::Instant::now();
    let mut last_batch = std::time::Instant::now();
    let mut last_scoring = -1.;
    let mut idle = 0u32;
    let disk_writer = lmu_pitwall::engineer::spool::BufferedWriter::new(root);
    loop {
        if stop.load(Ordering::Relaxed)
            || seconds.is_some_and(|s| start.elapsed().as_secs_f64() * speed >= s as f64)
        {
            break;
        }
        let elapsed = start.elapsed().as_secs_f64() * speed;
        let sample = if demo {
            Some(demo_sample(elapsed, &c))
        } else if c.legacy {
            legacy.sample()
        } else {
            capture.sample()
        };
        if let Some(mut sample) = sample {
            idle = 0;
            if !demo && !c.legacy && samples.is_empty() {
                sample.supplemental = supplement.read(sample.authoritative && c.rest_supplement);
            }
            if sample.game_s < last_scoring || sample.game_s - last_scoring >= 0.19 {
                last_scoring = sample.game_s;
            } else {
                sample.standings.clear();
            }
            samples.push(sample);
        } else {
            idle += 1;
            if idle > hz * 3 {
                supplement.active.store(false, Ordering::Relaxed);
                capture.reconnect();
                legacy.reconnect();
                idle = 0;
                write_status(
                    root,
                    "Waiting for LMU",
                    "Start a session with plugins enabled. Capture resumes automatically.",
                );
            }
        }
        if !samples.is_empty()
            && (samples.len() >= hz as usize || last_batch.elapsed() >= Duration::from_secs(1))
        {
            last_batch = std::time::Instant::now();
            seq += 1;
            let b = Batch {
                version: PROTOCOL,
                event_id: c.event_id.clone(),
                entry_id: c.entry_id.clone(),
                source_id: c.source_id.clone(),
                run_id: run.clone(),
                seq,
                historical: false,
                samples: std::mem::take(&mut samples),
            };
            disk_writer.enqueue(b)?;
        }
        tick += Duration::from_secs_f64(1. / hz as f64);
        if tick > std::time::Instant::now() {
            std::thread::sleep(tick - std::time::Instant::now())
        } else {
            tick = std::time::Instant::now();
        }
    }
    if !samples.is_empty() {
        seq += 1;
        let b = Batch {
            version: PROTOCOL,
            event_id: c.event_id,
            entry_id: c.entry_id,
            source_id: c.source_id,
            run_id: run.clone(),
            seq,
            historical: false,
            samples,
        };
        disk_writer.enqueue(b)?;
    }
    disk_writer.finish()?;
    Ok(())
}
fn demo_sample(t: f64, c: &AgentConfig) -> Sample {
    let lap = (t / 112.) as i32;
    let p = t % 112. / 112.;
    let angle = p * std::f64::consts::TAU;
    let corner = (angle * 7.).sin().max(0.);
    let speed = 290. - corner * 175.;
    let fuel = 100. - (t / 112. * 3.1) % 85.;
    let wheel = Wheel {
        temperature_c: Some([91., 88., 85.]),
        carcass_c: Some(85.),
        pressure_kpa: Some(160.),
        remaining: Some(1. - (t % 3000.) / 10000.),
        brake_c: Some(400. + corner * 250.),
        ..Default::default()
    };
    Sample {
        captured_ms: now_ms(),
        game_s: t,
        track: "Monza (SYNTHETIC)".into(),
        track_length_m: 5793.,
        session_kind: 10,
        session_remaining_s: Some((21600. - t).max(0.)),
        phase: 5,
        slot: Some(1),
        driver: c.label.clone(),
        vehicle: "Synthetic GT3".into(),
        class: "LMGT3".into(),
        lap,
        lap_distance_m: p * 5793.,
        lap_start_s: lap as f64 * 112.,
        last_lap_s: Some(112.),
        authoritative: true,
        valid_lap: true,
        speed_kmh: speed,
        rpm: 7000.,
        gear: if speed > 200. { 6 } else { 3 },
        throttle: 1. - corner,
        brake: corner * 0.7,
        steering: angle.sin() * 0.3,
        x: angle.sin() * 600.,
        z: angle.cos() * 1000.,
        fuel_l: Some(fuel),
        fuel_capacity_l: Some(100.),
        ve: Some(1. - (t / 112. * 0.08) % 0.9),
        soc: Some(0.7),
        water_c: Some(90.),
        oil_c: Some(100.),
        wheels: std::array::from_fn(|_| wheel.clone()),
        air_c: 24.,
        track_c: 31.,
        supplemental: Some(serde_json::json!({"synthetic":true})),
        standings: vec![Standing {
            slot: 1,
            driver: c.label.clone(),
            vehicle: "Synthetic GT3".into(),
            class: "LMGT3".into(),
            position: 1,
            laps: lap,
            distance_m: p * 5793.,
            last_lap_s: Some(112.),
            x: angle.sin() * 600.,
            z: angle.cos() * 1000.,
            ..Default::default()
        }],
        ..Default::default()
    }
}
