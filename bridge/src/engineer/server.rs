use super::{
    analysis::{compare_sections, compatible, Entry},
    model::*,
    storage::Storage,
    transport,
};
use anyhow::{ensure, Result};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use subtle::ConstantTimeEq;
use tokio::{
    net::TcpListener,
    time::{timeout, Duration},
};
use tokio_rustls::{
    rustls::{
        pki_types::{CertificateDer, PrivatePkcs8KeyDer},
        ServerConfig,
    },
    TlsAcceptor,
};

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Credential {
    pub source_id: String,
    pub entry_id: String,
    pub event_id: String,
    pub label: String,
    pub token: String,
    pub revoked: bool,
    #[serde(default)]
    pub shared_package: bool,
    #[serde(default)]
    pub parent_source_id: Option<String>,
}
pub struct App {
    pub storage: Storage,
    pub entries: BTreeMap<String, Entry>,
    pub rules: Rules,
    pub event_id: String,
    pub credentials: Vec<Credential>,
    pub cert: Vec<u8>,
    pub started: u64,
    pub errors: Vec<String>,
    pub live_batches: u64,
    pub recorded_bytes: u64,
    pub backfill_dirty: BTreeMap<String, u64>,
    pub source_seen: BTreeMap<String, u64>,
}
pub type Shared = Arc<Mutex<App>>;
impl App {
    fn revoke_source(&mut self, source: &str) -> Result<()> {
        let mut credentials = self.credentials.clone();
        for credential in &mut credentials {
            if credential.source_id == source
                || credential.parent_source_id.as_deref() == Some(source)
            {
                credential.revoked = true;
            }
        }
        self.storage.save("credentials", &credentials)?;
        self.credentials = credentials;
        Ok(())
    }
    fn authenticate(&mut self, hello: &Hello) -> Result<Credential> {
        ensure!(hello.version == PROTOCOL, "Unsupported collector protocol");
        if let Some(known) = self
            .credentials
            .iter()
            .find(|c| c.source_id == hello.source_id)
            .cloned()
        {
            ensure!(
                !known.revoked
                    && !known.shared_package
                    && bool::from(known.token.as_bytes().ct_eq(hello.token.as_bytes())),
                "Authentication rejected"
            );
            if let Some(parent) = &known.parent_source_id {
                ensure!(
                    self.credentials
                        .iter()
                        .any(|c| c.source_id == *parent && c.shared_package && !c.revoked),
                    "Shared package revoked"
                );
            }
            return Ok(known);
        }
        let parent_id = hello
            .enrollment_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Authentication rejected"))?;
        let parent = self
            .credentials
            .iter()
            .find(|c| {
                c.source_id == *parent_id
                    && c.shared_package
                    && !c.revoked
                    && bool::from(c.token.as_bytes().ct_eq(hello.token.as_bytes()))
            })
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Authentication rejected"))?;
        ensure!(
            hello.source_id.len() == 32 && hello.source_id.chars().all(|c| c.is_ascii_hexdigit()),
            "Invalid PC identity"
        );
        ensure!(
            self.credentials.len() < 128
                && self
                    .credentials
                    .iter()
                    .filter(|c| c.parent_source_id.as_ref() == Some(parent_id))
                    .count()
                    < 16,
            "Shared package device limit reached; export a new package"
        );
        let name = hello.device_name.as_deref().unwrap_or("Driver PC");
        ensure!(
            !name.is_empty() && name.chars().count() <= 64 && !name.chars().any(char::is_control),
            "Invalid PC name"
        );
        let child = Credential {
            source_id: hello.source_id.clone(),
            entry_id: parent.entry_id,
            event_id: parent.event_id,
            label: format!("{} / {}", parent.label, name),
            token: parent.token,
            parent_source_id: Some(parent_id.clone()),
            ..Default::default()
        };
        let mut credentials = self.credentials.clone();
        credentials.push(child.clone());
        self.storage.save("credentials", &credentials)?;
        self.credentials = credentials;
        Ok(child)
    }
    pub fn open(root: PathBuf) -> Result<Shared> {
        let storage = Storage::open(&root)?;
        let rules = storage.load("rules")?.unwrap_or_default();
        let event_id = storage.load("event_id")?.unwrap_or_else(new_id);
        storage.save("event_id", &event_id)?;
        let credentials = storage.load("credentials")?.unwrap_or_default();
        let (cert, _) = transport::certificate(&root)?;
        let mut a = Self {
            storage,
            rules,
            event_id,
            credentials,
            cert,
            entries: BTreeMap::new(),
            started: now_ms(),
            errors: vec![],
            live_batches: 0,
            recorded_bytes: 0,
            backfill_dirty: BTreeMap::new(),
            source_seen: BTreeMap::new(),
        };
        for (path, offset, size) in a.storage.batch_index(&a.event_id, None, 1000000)? {
            match a.storage.read(&path, offset, size) {
                Ok(b) => a.apply(&b, 0),
                Err(err) => {
                    a.errors
                        .push(format!("Skipped damaged archive record: {err}"));
                    if a.errors.len() > 20 {
                        a.errors.remove(0);
                    }
                }
            }
        }
        for entry in a.entries.values_mut() {
            entry.received = 0;
        }
        Ok(Arc::new(Mutex::new(a)))
    }
    fn apply(&mut self, b: &Batch, received: u64) {
        let old_lap = self
            .entries
            .get(&b.entry_id)
            .and_then(|e| e.laps.back())
            .map(|l| l.id.clone());
        for s in &b.samples {
            let stamp = if received == 0 {
                s.captured_ms
            } else {
                received
            };
            if let Some(e) = self.entries.get_mut(&b.entry_id) {
                for event in e.update(&b.source_id, s.clone(), stamp) {
                    if let Err(err) = self.storage.event(&event) {
                        self.errors.push(format!("Event recording failed: {err}"));
                    }
                }
            } else {
                self.entries.insert(
                    b.entry_id.clone(),
                    Entry::new(b.entry_id.clone(), b.source_id.clone(), s.clone(), stamp),
                );
            }
        }
        if let Some(lap) = self
            .entries
            .get(&b.entry_id)
            .and_then(|e| e.laps.back())
            .filter(|l| Some(&l.id) != old_lap.as_ref())
        {
            if let Err(err) = self.storage.save_lap(&self.event_id, lap) {
                self.errors.push(format!("Lap archive: {err}"));
            }
        }
    }
    pub fn ingest(&mut self, b: &Batch) -> Result<()> {
        ensure!(
            b.version == PROTOCOL && b.event_id == self.event_id,
            "wrong protocol or event"
        );
        ensure!(
            !b.samples.is_empty() && b.samples.len() <= 200,
            "batch sample limit"
        );
        ensure!(b.samples.iter().all(Sample::validate), "invalid sample");
        ensure!(
            b.samples
                .windows(2)
                .all(|w| w[1].captured_ms >= w[0].captured_ms),
            "unordered batch"
        );
        let wrote = self.storage.append(b)?;
        if wrote && b.historical {
            self.backfill_dirty.insert(b.entry_id.clone(), now_ms());
        }
        if wrote && !b.historical {
            let s = &b.samples[0];
            let newer = self.entries.get(&b.entry_id).is_none_or(|e| {
                s.captured_ms >= e.last.captured_ms
                    || (s.authoritative
                        && s.game_s >= e.last.game_s
                        && s.session_kind == e.last.session_kind
                        && s.track == e.last.track)
            });
            if newer {
                self.apply(b, now_ms());
                self.live_batches += 1;
            }
        }
        Ok(())
    }
}
// Reconstruct completed laps and drive times after offline backfill without replacing
// fresh values with history. Disk decoding runs outside the ingestion mutex.
fn start_archive_reconciliation(shared: Shared) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let work = {
                let mut a = lock(&shared);
                let candidate = a
                    .backfill_dirty
                    .iter()
                    .find(|(_, at)| now_ms().saturating_sub(**at) > 2000)
                    .map(|(id, _)| id.clone());
                candidate.map(|id| {
                    a.backfill_dirty.remove(&id);
                    (a.storage.root.clone(), a.event_id.clone(), id)
                })
            };
            let Some((root, event, id)) = work else {
                continue;
            };
            let task_id = id.clone();
            let task_event = event.clone();
            let rebuilt = tokio::task::spawn_blocking(move || -> Result<Option<Entry>> {
                let storage = Storage::open(&root)?;
                let mut entry: Option<Entry> = None;
                for (p, o, size) in storage.batch_index(&task_event, Some(&task_id), 1000000)? {
                    let b = storage.read(&p, o, size)?;
                    let previous = entry
                        .as_ref()
                        .and_then(|e| e.laps.back())
                        .map(|l| l.id.clone());
                    for sample in b.samples {
                        if let Some(e) = entry.as_mut() {
                            for event in e.update(&b.source_id, sample.clone(), sample.captured_ms)
                            {
                                storage.event(&event)?;
                            }
                        } else {
                            entry = Some(Entry::new(
                                task_id.clone(),
                                b.source_id.clone(),
                                sample.clone(),
                                sample.captured_ms,
                            ));
                        }
                    }
                    if let Some(lap) = entry
                        .as_ref()
                        .and_then(|e| e.laps.back())
                        .filter(|l| Some(&l.id) != previous.as_ref())
                    {
                        storage.save_lap(&task_event, lap)?;
                    }
                }
                Ok(entry)
            })
            .await;
            match rebuilt {
                Ok(Ok(Some(mut rebuilt))) => {
                    let mut a = lock(&shared);
                    let receipt = a.entries.get(&id).map(|e| e.received).unwrap_or(0);
                    // Replay the small live tail received while the archive was being decoded.
                    if let Ok(tail) = a.storage.batches(&event, Some(&id), 180) {
                        for b in tail {
                            for sample in b.samples {
                                if sample.captured_ms > rebuilt.last.captured_ms {
                                    rebuilt.update(
                                        &b.source_id,
                                        sample.clone(),
                                        sample.captured_ms,
                                    );
                                }
                            }
                        }
                    }
                    if a.entries
                        .get(&id)
                        .is_none_or(|e| rebuilt.last.captured_ms >= e.last.captured_ms)
                    {
                        rebuilt.received = receipt;
                        a.entries.insert(id, rebuilt);
                    } else {
                        a.backfill_dirty.insert(id, now_ms());
                    }
                }
                Ok(Ok(None)) => {}
                result => {
                    let mut a = lock(&shared);
                    a.errors.push(format!(
                        "Backfill reconciliation failed: {}",
                        match result {
                            Ok(Err(e)) => e.to_string(),
                            Err(e) => e.to_string(),
                            _ => String::new(),
                        }
                    ));
                    if a.errors.len() > 20 {
                        a.errors.remove(0);
                    }
                }
            }
        }
    });
}

type ApiResult = std::result::Result<Response, (StatusCode, String)>;
fn fail(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, e.to_string())
}
fn lock(s: &Shared) -> std::sync::MutexGuard<'_, App> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}
pub async fn run(shared: Shared, http_port: u16, ingest_port: u16, no_browser: bool) -> Result<()> {
    run_with_stop(
        shared,
        http_port,
        ingest_port,
        no_browser,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .await
}
pub async fn run_with_stop(
    shared: Shared,
    http_port: u16,
    ingest_port: u16,
    no_browser: bool,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    start_archive_reconciliation(shared.clone());
    let key = {
        let a = lock(&shared);
        std::fs::read(a.storage.root.join("receiver-key.der"))?
    };
    let cert = { lock(&shared).cert.clone() };
    let tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert)],
            PrivatePkcs8KeyDer::from(key).into(),
        )?;
    let acceptor = TlsAcceptor::from(Arc::new(tls));
    let listener = TcpListener::bind(("0.0.0.0", ingest_port)).await?;
    let remote = shared.clone();
    tokio::spawn(async move {
        let permits = Arc::new(tokio::sync::Semaphore::new(32));
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let acceptor = acceptor.clone();
            let app = remote.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let result: Result<()> = async {
                    let mut tls =
                        timeout(Duration::from_secs(10), acceptor.accept(stream)).await??;
                    let hello: Hello =
                        timeout(Duration::from_secs(10), transport::receive(&mut tls)).await??;
                    let credential = { lock(&app).authenticate(&hello)? };
                    lock(&app)
                        .source_seen
                        .insert(credential.source_id.clone(), now_ms());
                    transport::send(
                        &mut tls,
                        &Ack {
                            seq: 0,
                            accepted: true,
                            error: None,
                        },
                    )
                    .await?;
                    loop {
                        let b: Batch =
                            timeout(Duration::from_secs(30), transport::receive(&mut tls))
                                .await??;
                        ensure!(
                            b.source_id == credential.source_id
                                && b.entry_id == credential.entry_id
                                && b.event_id == credential.event_id,
                            "identity mismatch"
                        );
                        let result = {
                            let mut a = lock(&app);
                            ensure!(
                                a.credentials
                                    .iter()
                                    .any(|c| c.source_id == credential.source_id && !c.revoked),
                                "credential revoked"
                            );
                            a.source_seen.insert(credential.source_id.clone(), now_ms());
                            if b.samples.is_empty()
                                && b.seq == 0
                                && b.run_id == "heartbeat"
                                && b.version == PROTOCOL
                            {
                                Ok(())
                            } else {
                                a.ingest(&b)
                            }
                        };
                        let ack = Ack {
                            seq: b.seq,
                            accepted: result.is_ok(),
                            error: result.err().map(|e| e.to_string()),
                        };
                        transport::send(&mut tls, &ack).await?;
                    }
                }
                .await;
                if let Err(err) = result {
                    let msg = err.to_string();
                    let disconnect = err.downcast_ref::<std::io::Error>().is_some_and(|e| {
                        matches!(
                            e.kind(),
                            std::io::ErrorKind::UnexpectedEof
                                | std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::BrokenPipe
                        )
                    });
                    // Frames are length-delimited and acknowledged only after durable
                    // storage. A lost/truncated connection is retried by the agent.
                    if !disconnect {
                        let mut a = lock(&app);
                        a.errors.push(msg);
                        if a.errors.len() > 20 {
                            a.errors.remove(0);
                        }
                    }
                }
            });
        }
    });
    let app = Router::new()
        .route("/api/state", get(state))
        .route("/api/rules", get(rules).post(save_rules))
        .route("/api/pair", post(pair))
        .route("/api/revoke/{source}", post(revoke))
        .route("/api/events", get(events).post(add_event))
        .route("/api/replay/{entry}", get(replay))
        .route("/api/compare", get(compare))
        .route("/api/export/{entry}", get(export_csv))
        .route("/api/import", post(import_lap))
        .route("/api/native/inspect", post(native_inspect))
        .route("/api/native/import", post(native_import))
        .route("/api/laps/{entry}", get(laps))
        .route("/api/lap/{id}", get(full_lap))
        .fallback(get(assets))
        .with_state(shared)
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| async move {
                let host = req
                    .headers()
                    .get(header::HOST)
                    .and_then(|s| s.to_str().ok())
                    .unwrap_or("");
                let allowed = host == format!("127.0.0.1:{http_port}")
                    || host == format!("localhost:{http_port}");
                let origin = req
                    .headers()
                    .get(header::ORIGIN)
                    .and_then(|s| s.to_str().ok());
                if !allowed
                    || origin.is_some_and(|o| {
                        o != format!("http://127.0.0.1:{http_port}")
                            && o != format!("http://localhost:{http_port}")
                    })
                {
                    return StatusCode::FORBIDDEN.into_response();
                }
                let mut response = next.run(req).await;
                response.headers_mut().insert(
                    "X-Content-Type-Options",
                    header::HeaderValue::from_static("nosniff"),
                );
                response.headers_mut().insert(
                    "Cache-Control",
                    header::HeaderValue::from_static("no-store"),
                );
                response
            },
        ));
    let local = TcpListener::bind(("127.0.0.1", http_port)).await?;
    if !no_browser {
        let _ = open::that(format!("http://127.0.0.1:{http_port}"));
    }
    axum::serve(local, app)
        .with_graceful_shutdown(async move {
            tokio::select!{
                _=tokio::signal::ctrl_c()=>{},
                _=async{while !stop.load(std::sync::atomic::Ordering::Relaxed){tokio::time::sleep(Duration::from_millis(100)).await;}}=>{}
            }
        })
        .await?;
    Ok(())
}
async fn state(State(s): State<Shared>) -> Json<Value> {
    let a = lock(&s);
    let now = now_ms();
    Json(
        json!({"version":PROTOCOL,"now_ms":now,"event_id":a.event_id,"rules":a.rules,"entries":a.entries.values().map(|e|e.view(&a.rules,now)).collect::<Vec<_>>(),"sources":a.credentials.iter().map(|c|json!({"source_id":c.source_id,"entry_id":c.entry_id,"label":c.label,"revoked":c.revoked,"shared_package":c.shared_package,"parent_source_id":c.parent_source_id,"last_seen_ms":a.source_seen.get(&c.source_id),"online":!c.revoked && a.source_seen.get(&c.source_id).is_some_and(|at|now.saturating_sub(*at)<=20000)})).collect::<Vec<_>>(),"errors":a.errors,"uptime_s":(now-a.started)/1000,"live_batches":a.live_batches,"data_directory":a.storage.root.to_string_lossy()}),
    )
}
async fn rules(State(s): State<Shared>) -> Json<Rules> {
    Json(lock(&s).rules.clone())
}
async fn save_rules(State(s): State<Shared>, Json(r): Json<Rules>) -> ApiResult {
    if !r.validate() {
        return Err(fail("Invalid event rules"));
    }
    let mut a = lock(&s);
    a.storage.save("rules", &r).map_err(fail)?;
    a.rules = r;
    Ok(Json(json!({"saved":true})).into_response())
}
#[derive(Deserialize)]
struct PairRequest {
    endpoint: String,
    entry_id: String,
    label: String,
    #[serde(default)]
    legacy: bool,
    #[serde(default)]
    rest_supplement: bool,
    #[serde(default)]
    shared_package: bool,
}
async fn pair(State(s): State<Shared>, Json(p): Json<PairRequest>) -> ApiResult {
    if p.endpoint.len() > 255
        || !p.endpoint.contains(':')
        || p.entry_id.is_empty()
        || p.entry_id.len() > 80
        || p.label.len() > 80
    {
        return Err(fail("Enter host:port, car entry and driver label"));
    }
    let (certificate, event_id) = {
        let a = lock(&s);
        if a.credentials.len() >= 128 {
            return Err(fail("Credential limit reached"));
        }
        (a.cert.clone(), a.event_id.clone())
    };
    let source_id = new_id();
    let token = new_id() + &new_id();
    let config = AgentConfig {
        endpoint: p.endpoint,
        certificate_der: base64::engine::general_purpose::STANDARD.encode(&certificate),
        token: token.clone(),
        source_id: source_id.clone(),
        event_id,
        entry_id: p.entry_id.clone(),
        label: p.label.clone(),
        legacy: p.legacy,
        rest_supplement: p.rest_supplement,
        shared_package: p.shared_package,
        enrollment_id: None,
    };
    let embedded_config = config.clone();
    let bytes=tokio::task::spawn_blocking(move||{let config=embedded_config;
    let agent_path = std::env::current_exe()
        .map_err(fail)?
        .parent()
        .unwrap()
        .join("lmu-agent.exe");
    let mut exe = std::fs::read(&agent_path).map_err(|_| {
        fail(format!(
            "Place lmu-agent.exe next to engineer EXE: {}",
            agent_path.display()
        ))
    })?;
    let config_bytes = serde_json::to_vec(&config).map_err(fail)?;
    exe.extend_from_slice(&config_bytes);
    exe.extend_from_slice(&(config_bytes.len() as u32).to_le_bytes());
    exe.extend_from_slice(b"LMUENG01");
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("lmu-agent.exe", opts).map_err(fail)?;
    zip.write_all(&exe).map_err(fail)?;
    zip.start_file("START-HERE.txt", opts).map_err(fail)?;
    zip.write_all(b"Extract this folder, then double-click lmu-agent.exe before driving. Keep it running through your stint. The agent reconnects automatically and records during outages. Close it from the tray/Task Manager after the race. Your telemetry goes only to your configured engineer. This package contains your private pairing credential; share only with the intended drivers of this car.\r\n").map_err(fail)?;
    Ok::<_,(StatusCode,String)>(zip.finish().map_err(fail)?.into_inner())
    }).await.map_err(fail)??;
    let mut a = lock(&s);
    if a.credentials.len() >= 128 {
        return Err(fail("Credential limit reached"));
    }
    a.credentials.push(Credential {
        source_id,
        entry_id: p.entry_id,
        event_id: config.event_id,
        label: p.label,
        token,
        revoked: false,
        shared_package: p.shared_package,
        parent_source_id: None,
    });
    a.storage
        .save("credentials", &a.credentials)
        .map_err(fail)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=LMU-driver-package.zip",
            ),
        ],
        bytes,
    )
        .into_response())
}
async fn revoke(State(s): State<Shared>, Path(source): Path<String>) -> ApiResult {
    let mut a = lock(&s);
    a.revoke_source(&source).map_err(fail)?;
    Ok(Json(json!({"revoked":true})).into_response())
}
async fn events(State(s): State<Shared>) -> ApiResult {
    Ok(Json(lock(&s).storage.events().map_err(fail)?).into_response())
}
#[derive(Deserialize)]
struct Note {
    entry_id: String,
    text: String,
    #[serde(default)]
    correction_of: Option<String>,
}
async fn add_event(State(s): State<Shared>, Json(n): Json<Note>) -> ApiResult {
    if n.text.len() > 2000 {
        return Err(fail("Note too long"));
    }
    let a = lock(&s);
    let e = RaceEvent {
        id: new_id(),
        at_ms: now_ms(),
        game_s: a
            .entries
            .get(&n.entry_id)
            .map(|e| e.last.game_s)
            .unwrap_or(0.),
        entry_id: n.entry_id,
        kind: if n.correction_of.is_some() {
            "correction"
        } else {
            "note"
        }
        .into(),
        text: format!(
            "{}{}",
            n.text,
            n.correction_of
                .map(|id| format!(" [corrects {id}]"))
                .unwrap_or_default()
        ),
        manual: true,
    };
    a.storage.event(&e).map_err(fail)?;
    Ok(Json(e).into_response())
}
#[derive(Deserialize)]
struct ReplayQuery {
    #[serde(default)]
    from_ms: Option<u64>,
    #[serde(default)]
    to_ms: Option<u64>,
    #[serde(default)]
    limit: Option<usize>,
}
async fn replay(
    State(s): State<Shared>,
    Path(entry): Path<String>,
    Query(q): Query<ReplayQuery>,
) -> ApiResult {
    let (root, index) = {
        let a = lock(&s);
        (
            a.storage.root.clone(),
            a.storage
                .batch_index(&a.event_id, Some(&entry), q.limit.unwrap_or(180).min(3600))
                .map_err(fail)?,
        )
    };
    let points = tokio::task::spawn_blocking(move || -> Result<Vec<Sample>> {
        let mut points = Vec::new();
        for (p, o, size) in index {
            let b = Storage::read_at(&root, &p, o, size)?;
            for (i, mut sample) in b.samples.into_iter().enumerate() {
                if i % 5 == 0
                    && q.from_ms.is_none_or(|t| sample.captured_ms >= t)
                    && q.to_ms.is_none_or(|t| sample.captured_ms <= t)
                {
                    sample.standings = Vec::new();
                    sample.supplemental = None;
                    points.push(sample);
                }
            }
        }
        Ok(points)
    })
    .await
    .map_err(fail)?
    .map_err(fail)?;
    Ok(Json(points).into_response())
}
async fn laps(State(s): State<Shared>, Path(entry): Path<String>) -> ApiResult {
    let a = lock(&s);
    Ok(Json(a.storage.lap_metadata(&a.event_id, &entry).map_err(fail)?).into_response())
}
async fn full_lap(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult {
    Ok(Json(lock(&s).storage.lap(&id).map_err(fail)?).into_response())
}
#[derive(Deserialize)]
struct CompareQuery {
    entry: String,
    current: String,
    reference: String,
    reference_entry: Option<String>,
}
async fn compare(State(s): State<Shared>, Query(q): Query<CompareQuery>) -> ApiResult {
    let a = lock(&s);
    let current = a.storage.lap(&q.current).map_err(fail)?;
    let reference = a.storage.lap(&q.reference).map_err(fail)?;
    if current.entry_id != q.entry || reference.entry_id != q.reference_entry.unwrap_or(q.entry) {
        return Err(fail("Lap entry does not match selection"));
    }
    Ok(Json(json!({"compatible":compatible(&current,&reference),"sections":compare_sections(&current,&reference,&super::analysis::sections_for(&reference,&a.rules)),"current":current,"reference":reference})).into_response())
}
async fn import_lap(State(s): State<Shared>, Json(mut lap): Json<Lap>) -> ApiResult {
    if lap.samples.len() > 20000
        || lap.samples.is_empty()
        || !lap.time_s.is_finite()
        || lap.time_s <= 0.
        || !lap.samples.iter().all(Sample::validate)
    {
        return Err(fail("Invalid lap import"));
    }
    lap.id = new_id();
    lap.reference_only = true;
    let a = lock(&s);
    if !a.entries.contains_key(&lap.entry_id) {
        return Err(fail("Choose an existing car entry"));
    }
    a.storage.save_lap(&a.event_id, &lap).map_err(fail)?;
    Ok(Json(lap).into_response())
}
#[derive(Deserialize)]
struct NativePath {
    path: String,
}
async fn native_inspect(Json(p): Json<NativePath>) -> ApiResult {
    let result = tokio::task::spawn_blocking(move || super::native_recording::inspect(&p.path))
        .await
        .map_err(fail)?
        .map_err(fail)?;
    Ok(Json(result).into_response())
}
async fn native_import(
    State(s): State<Shared>,
    Json(p): Json<super::native_recording::ImportRequest>,
) -> ApiResult {
    if !lock(&s).entries.contains_key(&p.entry_id) {
        return Err(fail("Choose an existing car entry"));
    }
    let lap = tokio::task::spawn_blocking(move || super::native_recording::import(&p))
        .await
        .map_err(fail)?
        .map_err(fail)?;
    let a = lock(&s);
    a.storage.save_lap(&a.event_id, &lap).map_err(fail)?;
    Ok(Json(lap).into_response())
}

async fn export_csv(State(s): State<Shared>, Path(entry): Path<String>) -> ApiResult {
    let (root, index) = {
        let a = lock(&s);
        (
            a.storage.root.clone(),
            a.storage
                .batch_index(&a.event_id, Some(&entry), 1000000)
                .map_err(fail)?,
        )
    };
    // Bounded streaming keeps full-race exports off the live ingestion mutex.
    let (tx, rx) = tokio::sync::mpsc::channel::<std::result::Result<String, std::io::Error>>(4);
    tokio::task::spawn_blocking(move || {
        let result: Result<()> = (|| {
            let storage = Storage::open(&root)?;
            tx.blocking_send(Ok(super::csv_export::header()))
                .map_err(|_| anyhow::anyhow!("Export closed"))?;
            for (p, o, size) in index {
                let b = storage.read(&p, o, size)?;
                let mut csv = String::new();
                for s in &b.samples {
                    super::csv_export::row(&mut csv, &b, s)?;
                }
                if tx.blocking_send(Ok(csv)).is_err() {
                    return Ok(());
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
        }
    });
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=telemetry.csv",
            ),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

#[derive(rust_embed::RustEmbed)]
#[folder = "../dashboard/dist/"]
struct Assets;
async fn disk_asset(root: &std::path::Path, path: &str) -> Option<Vec<u8>> {
    // External UI files are public assets only; never resolve a traversal or
    // a symlink outside the UI directory into the receiver's private profile.
    if !std::path::Path::new(path)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    let root = tokio::fs::canonicalize(root).await.ok()?;
    let file = tokio::fs::canonicalize(root.join(path)).await.ok()?;
    if !file.starts_with(&root) {
        return None;
    }
    let metadata = tokio::fs::metadata(&file).await.ok()?;
    if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
        return None;
    }
    tokio::fs::read(file).await.ok()
}
async fn assets(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() || path == "legacy" {
        "index.html"
    } else {
        path
    };
    let external = match std::env::current_exe() {
        Ok(exe) => disk_asset(&exe.with_file_name("ui"), path).await,
        Err(_) => None,
    };
    let content = external.or_else(|| Assets::get(path).map(|asset| asset.data.into_owned()));
    match content {
        Some(f) => {
            let mime = if path.ends_with(".js") {
                "application/javascript"
            } else if path.ends_with(".css") {
                "text/css"
            } else if path.ends_with(".svg") {
                "image/svg+xml"
            } else if path.ends_with(".woff2") {
                "font/woff2"
            } else if path.ends_with(".woff") {
                "font/woff"
            } else if path.ends_with(".png") {
                "image/png"
            } else if path.ends_with(".json") {
                "application/json"
            } else {
                "text/html"
            };
            ([(header::CONTENT_TYPE, mime)], Body::from(f)).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod live_ui_tests {
    use super::*;
    #[test]
    fn shared_package_enrolls_three_pcs_once_and_revokes_the_whole_group() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path().into()).unwrap();
        let parent = new_id();
        let key = new_id();
        let mut hellos = Vec::new();
        {
            let mut a = lock(&app);
            let event = a.event_id.clone();
            a.credentials.push(Credential {
                source_id: parent.clone(),
                token: key.clone(),
                entry_id: "Race car".into(),
                event_id: event,
                label: "Team".into(),
                shared_package: true,
                ..Default::default()
            });
            for n in 0..3 {
                let hello = Hello {
                    version: PROTOCOL,
                    source_id: new_id(),
                    token: key.clone(),
                    enrollment_id: Some(parent.clone()),
                    device_name: Some(format!("PC {n}")),
                };
                let first = a.authenticate(&hello).unwrap();
                assert_eq!(first.entry_id, "Race car");
                assert_eq!(a.authenticate(&hello).unwrap().source_id, first.source_id);
                hellos.push(hello);
            }
            assert_eq!(a.credentials.len(), 4);
        }
        let restored = App::open(dir.path().into()).unwrap();
        let mut a = lock(&restored);
        assert_eq!(a.credentials.len(), 4);
        for hello in &hellos {
            assert!(a.authenticate(hello).is_ok());
        }
        a.revoke_source(&parent).unwrap();
        assert!(a.credentials.iter().all(|c| c.revoked));
        for hello in &hellos {
            assert!(a.authenticate(hello).is_err());
        }
        let fresh = Hello {
            version: PROTOCOL,
            token: key,
            source_id: new_id(),
            enrollment_id: Some(parent),
            device_name: Some("New PC".into()),
        };
        assert!(a.authenticate(&fresh).is_err());
    }
    #[test]
    fn shared_enrollment_requires_the_key_and_does_not_allow_template_login() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path().into()).unwrap();
        let mut a = lock(&app);
        let parent = new_id();
        a.credentials.push(Credential {
            source_id: parent.clone(),
            token: "private-key".into(),
            shared_package: true,
            ..Default::default()
        });
        let mut hello = Hello {
            version: PROTOCOL,
            source_id: new_id(),
            token: "wrong-key".into(),
            enrollment_id: Some(parent.clone()),
            device_name: Some("PC".into()),
        };
        assert!(a.authenticate(&hello).is_err());
        hello.token = "private-key".into();
        hello.source_id = parent;
        assert!(a.authenticate(&hello).is_err());
        hello.source_id = new_id();
        hello.device_name = Some("bad\nname".into());
        assert!(a.authenticate(&hello).is_err());
        assert_eq!(a.credentials.len(), 1);
    }
    #[tokio::test]
    async fn disk_ui_reads_updates_and_rejects_paths_outside_its_root() {
        let dir = tempfile::tempdir().unwrap();
        let ui = dir.path().join("ui");
        std::fs::create_dir(&ui).unwrap();
        std::fs::write(dir.path().join("private.key"), "private").unwrap();
        std::fs::write(ui.join("index.html"), "first").unwrap();
        assert_eq!(disk_asset(&ui, "index.html").await.unwrap(), b"first");
        std::fs::write(ui.join("index.html"), "second").unwrap();
        assert_eq!(disk_asset(&ui, "index.html").await.unwrap(), b"second");
        for path in [
            "../private.key",
            "..\\private.key",
            "/private.key",
            "C:\\private.key",
            "missing.js",
            "",
        ] {
            assert!(disk_asset(&ui, path).await.is_none(), "{path}");
        }
    }
}
