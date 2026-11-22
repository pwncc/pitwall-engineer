//! Atomic local batches; valid interrupted writes can be resumed after a crash.
use super::model::*;
use anyhow::{ensure, Result};
use std::{io::Write, path::Path};
const RAW_LIMIT: u64 = 16 * 1024 * 1024;
const QUEUED_BATCHES: usize = 30;

/// Keep filesystem flushes off the sampling thread. The bounded queue holds
/// at most about thirty seconds; pressure beyond that blocks without dropping
/// an acquired batch. Finishing drains every queued batch before reporting done.
pub struct BufferedWriter {
    sender: Option<std::sync::mpsc::SyncSender<Batch>>,
    worker: Option<std::thread::JoinHandle<Result<()>>>,
}
impl BufferedWriter {
    pub fn new(root: &Path) -> Self {
        let root = root.to_owned();
        let mut checked = std::time::Instant::now();
        Self::with_sink(move |batch| {
            write(&root, &batch)?;
            if checked.elapsed() >= std::time::Duration::from_secs(30) {
                let bytes: u64 = std::fs::read_dir(root.join("spool"))?
                    .filter_map(|entry| entry.ok()?.metadata().ok().map(|m| m.len()))
                    .sum();
                ensure!(bytes < 4 * 1024 * 1024 * 1024,
                    "Offline spool reached 4 GiB; preserve files and reconnect before restarting capture");
                checked = std::time::Instant::now();
            }
            Ok(())
        })
    }
    fn with_sink(mut sink: impl FnMut(Batch) -> Result<()> + Send + 'static) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(QUEUED_BATCHES);
        let worker = std::thread::spawn(move || {
            for batch in receiver {
                sink(batch)?;
            }
            Ok(())
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
        }
    }
    pub fn enqueue(&self, batch: Batch) -> Result<()> {
        self.sender.as_ref().unwrap().send(batch).map_err(|_| {
            anyhow::anyhow!("Telemetry disk writer stopped; check free space and the collector log")
        })
    }
    pub fn finish(mut self) -> Result<()> {
        self.drain()
    }
    fn drain(&mut self) -> Result<()> {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Telemetry disk writer panicked"))??;
        }
        Ok(())
    }
}
impl Drop for BufferedWriter {
    fn drop(&mut self) {
        if let Err(error) = self.drain() {
            tracing::error!("Telemetry disk writer: {error:#}");
        }
    }
}
pub fn write(root: &Path, b: &Batch) -> Result<()> {
    let captured = b.samples.first().map(|s| s.captured_ms).unwrap_or(0);
    ensure!(
        b.run_id.len() == 32 && b.run_id.chars().all(|c| c.is_ascii_hexdigit()),
        "Invalid local run identifier"
    );
    let name = format!("{captured:020}-{}-{:012}", b.run_id, b.seq);
    let temporary = root.join("spool").join(format!("{name}.partial"));
    let final_path = temporary.with_extension("batch");
    let bytes = rmp_serde::to_vec_named(b)?;
    ensure!(bytes.len() as u64 <= RAW_LIMIT, "Local batch exceeds limit");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_data()?;
    drop(file);
    std::fs::rename(temporary, final_path)?;
    Ok(())
}
pub fn recover(root: &Path, c: &AgentConfig) -> Result<usize> {
    let mut recovered = 0;
    for item in std::fs::read_dir(root.join("spool"))? {
        let path = item?.path();
        if !path.extension().is_some_and(|s| s == "partial") || !path.is_file() {
            continue;
        }
        if std::fs::metadata(&path)?.len() > RAW_LIMIT {
            continue;
        }
        let raw = std::fs::read(&path)?;
        let Ok(b) = rmp_serde::from_slice::<Batch>(&raw) else {
            continue;
        };
        if b.version != PROTOCOL
            || b.source_id != c.source_id
            || b.entry_id != c.entry_id
            || b.event_id != c.event_id
            || b.samples.is_empty()
            || b.samples.len() > 200
            || !b.samples.iter().all(Sample::validate)
        {
            continue;
        }
        let final_path = path.with_extension("batch");
        if !final_path.exists() {
            std::fs::rename(&path, final_path)?;
            recovered += 1;
        }
    }
    Ok(recovered)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffered_writer_retains_order_and_drains_after_a_blocked_disk() {
        let (release, wait) = std::sync::mpsc::channel();
        let (entered, blocked) = std::sync::mpsc::channel();
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = received.clone();
        let writer = BufferedWriter::with_sink(move |batch| {
            if batch.seq == 1 {
                entered.send(()).unwrap();
                wait.recv().unwrap();
            }
            observed.lock().unwrap().push(batch.seq);
            Ok(())
        });
        let batch = |seq| Batch {
            version: PROTOCOL,
            event_id: "e".into(),
            entry_id: "c".into(),
            source_id: "s".into(),
            run_id: new_id(),
            seq,
            historical: false,
            samples: vec![Sample::default(); 50],
        };
        writer.enqueue(batch(1)).unwrap();
        blocked
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        for seq in 2..=QUEUED_BATCHES as u64 + 1 {
            writer.enqueue(batch(seq)).unwrap();
        }
        assert!(received.lock().unwrap().is_empty());
        release.send(()).unwrap();
        writer.finish().unwrap();
        assert_eq!(
            *received.lock().unwrap(),
            (1..=QUEUED_BATCHES as u64 + 1).collect::<Vec<_>>()
        );
    }
    #[test]
    fn buffered_writer_reports_storage_failure_on_finish() {
        let writer = BufferedWriter::with_sink(|_| anyhow::bail!("test disk failure"));
        writer
            .enqueue(Batch {
                version: PROTOCOL,
                event_id: "e".into(),
                entry_id: "c".into(),
                source_id: "s".into(),
                run_id: new_id(),
                seq: 1,
                historical: false,
                samples: vec![],
            })
            .unwrap();
        assert!(writer
            .finish()
            .unwrap_err()
            .to_string()
            .contains("test disk failure"));
    }
    #[test]
    fn complete_partial_is_recovered_but_damaged_data_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("spool")).unwrap();
        let c = AgentConfig {
            endpoint: "unused".into(),
            certificate_der: "unused".into(),
            token: "unused".into(),
            source_id: new_id(),
            event_id: new_id(),
            entry_id: "car".into(),
            label: "test".into(),
            legacy: false,
            rest_supplement: false,
            ..Default::default()
        };
        let b = Batch {
            version: 1,
            event_id: c.event_id.clone(),
            entry_id: c.entry_id.clone(),
            source_id: c.source_id.clone(),
            run_id: new_id(),
            seq: 1,
            historical: false,
            samples: vec![Sample::default()],
        };
        write(root.path(), &b).unwrap();
        let saved = std::fs::read_dir(root.path().join("spool"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let partial = saved.with_extension("partial");
        std::fs::rename(&saved, &partial).unwrap();
        let damaged = root.path().join("spool/damaged.partial");
        std::fs::write(&damaged, [1, 2, 3]).unwrap();
        assert_eq!(recover(root.path(), &c).unwrap(), 1);
        assert!(saved.exists());
        assert!(damaged.exists());
        assert!(!partial.exists());
    }
}
