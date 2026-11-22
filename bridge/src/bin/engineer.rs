#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use clap::Parser;
use lmu_pitwall::engineer::{model::*, server, tray};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    name = "Pitwall Engineer",
    version = include_str!("../../../ENGINEER-VERSION").trim(),
    about = "Local multi-car race engineering workstation for LMU"
)]
struct Args {
    #[arg(long)]
    data: Option<PathBuf>,
    #[arg(long, default_value_t = 7437)]
    http_port: u16,
    #[arg(long, default_value_t = 7443)]
    ingest_port: u16,
    #[arg(long)]
    no_browser: bool,
    #[arg(long)]
    no_tray: bool,
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let message = format!("LMU Engineer could not start: {e:#}");
        let path = std::env::temp_dir().join("lmu-engineer-error.txt");
        let _ = std::fs::write(&path, &message);
        let _ = open::that(&path);
    }
}
async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let root = args.data.unwrap_or_else(|| {
        PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into()))
            .join("LMUEngineer")
    });
    std::fs::create_dir_all(&root)?;
    let singleton = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("receiver.lock"))?;
    match singleton.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            if !args.no_browser {
                let _ = open::that(format!("http://127.0.0.1:{}", args.http_port));
            }
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }

    let file = tracing_appender::rolling::daily(root.join("logs"), "receiver.log");
    let (writer, _guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::fmt().with_writer(writer).init();
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let app = server::App::open(root)?;
    tracing::info!("Receiver started at {}", now_ms());
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if !args.no_tray {
        tray::start_receiver(format!("http://127.0.0.1:{}", args.http_port), stop.clone());
    }
    server::run_with_stop(app, args.http_port, args.ingest_port, args.no_browser, stop).await
}
