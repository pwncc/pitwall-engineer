use super::model::*;
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub struct Storage {
    pub db: Connection,
    pub root: PathBuf,
}
impl Storage {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root.join("chunks"))?;
        let db = Connection::open(root.join("race.sqlite"))?;
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS batches (id INTEGER PRIMARY KEY, source TEXT, run TEXT, seq INTEGER, event TEXT, entry_id TEXT, captured INTEGER, path TEXT, offset INTEGER, size INTEGER, UNIQUE(source,run,seq));
          CREATE INDEX IF NOT EXISTS batch_entry ON batches(event,entry_id,id);
          CREATE INDEX IF NOT EXISTS batch_capture_entry ON batches(event,entry_id,captured,id);
          CREATE INDEX IF NOT EXISTS batch_capture_event ON batches(event,captured,id);
          CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, data TEXT);
          CREATE TABLE IF NOT EXISTS laps (id TEXT PRIMARY KEY, event TEXT, entry_id TEXT, metadata TEXT, path TEXT);
          CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, data TEXT);")?;
        Ok(Self {
            db,
            root: root.into(),
        })
    }
    pub fn contains(&self, b: &Batch) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM batches WHERE source=? AND run=? AND seq=?)",
            params![b.source_id, b.run_id, b.seq],
            |r| r.get(0),
        )?)
    }
    pub fn append(&mut self, b: &Batch) -> Result<bool> {
        if self.contains(b)? {
            return Ok(false);
        }
        ensure!(
            b.samples.len() <= 200 && !b.samples.is_empty(),
            "invalid batch count"
        );
        let encoded = rmp_serde::to_vec_named(b)?;
        let compressed = zstd::encode_all(encoded.as_slice(), 3)?;
        ensure!(compressed.len() <= 4 * 1024 * 1024, "batch too large");
        // Generated names only: agent identities never become filesystem paths.
        let bucket = b.samples[0].captured_ms / 3_600_000;
        use sha2::{Digest, Sha256};
        let source = hex::encode(Sha256::digest(b.source_id.as_bytes()));
        let path = format!("chunks/{}-{bucket}.zstframes", &source[..16]);
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join(&path))?;
        let offset = f.metadata()?.len();
        f.write_all(&(compressed.len() as u32).to_le_bytes())?;
        f.write_all(&compressed)?;
        f.sync_data()?;
        // Index only durable records. An interrupted index commit leaves an ignored tail record.
        self.db.execute("INSERT INTO batches(source,run,seq,event,entry_id,captured,path,offset,size) VALUES(?,?,?,?,?,?,?,?,?)",params![b.source_id,b.run_id,b.seq,b.event_id,b.entry_id,b.samples[0].captured_ms,path,offset,compressed.len()])?;
        Ok(true)
    }
    pub fn read(&self, path: &str, offset: u64, size: usize) -> Result<Batch> {
        Self::read_at(&self.root, path, offset, size)
    }
    pub fn read_at(root: &Path, path: &str, offset: u64, size: usize) -> Result<Batch> {
        ensure!(size <= 4 * 1024 * 1024, "invalid chunk size");
        let mut f = std::fs::File::open(root.join(path))?;
        f.seek(SeekFrom::Start(offset))?;
        let mut prefix = [0; 4];
        f.read_exact(&mut prefix)?;
        ensure!(
            u32::from_le_bytes(prefix) as usize == size,
            "chunk index mismatch"
        );
        let mut data = vec![0; size];
        f.read_exact(&mut data)?;
        let raw = decode_limited(&data)?;
        Ok(rmp_serde::from_slice(&raw)?)
    }
    pub fn batches(&self, event: &str, entry: Option<&str>, limit: usize) -> Result<Vec<Batch>> {
        self.batch_index(event, entry, limit)?
            .into_iter()
            .map(|(p, o, s)| self.read(&p, o, s))
            .collect()
    }
    pub fn batch_index(
        &self,
        event: &str,
        entry: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, u64, usize)>> {
        let limit = limit.min(1000000);
        let map = |r: &rusqlite::Row<'_>| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u64>(1)?,
                r.get::<_, usize>(2)?,
            ))
        };
        if let Some(entry) = entry {
            let mut q=self.db.prepare("SELECT path,offset,size FROM (SELECT id,captured,path,offset,size FROM batches WHERE event=? AND entry_id=? ORDER BY captured DESC,id DESC LIMIT ?) ORDER BY captured,id")?;
            let rows = q
                .query_map(params![event, entry, limit], map)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        } else {
            let mut q=self.db.prepare("SELECT path,offset,size FROM (SELECT id,captured,path,offset,size FROM batches WHERE event=? ORDER BY captured DESC,id DESC LIMIT ?) ORDER BY captured,id")?;
            let rows = q
                .query_map(params![event, limit], map)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
    }
    pub fn save<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO settings VALUES(?,?)",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
    pub fn load<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        use rusqlite::OptionalExtension;
        self.db
            .query_row("SELECT data FROM settings WHERE key=?", [key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .map(|v| serde_json::from_str(&v).context("saved setting"))
            .transpose()
    }
    pub fn event(&self, e: &RaceEvent) -> Result<()> {
        self.db.execute(
            "INSERT INTO events VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            params![e.id, serde_json::to_string(e)?],
        )?;
        Ok(())
    }
    pub fn events(&self) -> Result<Vec<RaceEvent>> {
        let mut q = self.db.prepare(
            "SELECT data FROM events ORDER BY json_extract(data,'$.at_ms') DESC LIMIT 500",
        )?;
        let rows = q.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn save_lap(&self, event: &str, lap: &Lap) -> Result<()> {
        ensure!(
            lap.id.chars().all(|c| c.is_ascii_hexdigit()),
            "invalid lap identifier"
        );
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM laps WHERE id=?)",
            [&lap.id],
            |r| r.get(0),
        )?;
        if exists || lap.samples.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(self.root.join("laps"))?;
        let path = format!("laps/{}.zst", lap.id);
        let raw = rmp_serde::to_vec_named(lap)?;
        let compressed = zstd::encode_all(raw.as_slice(), 3)?;
        let mut f = std::fs::File::create(self.root.join(&path))?;
        f.write_all(&compressed)?;
        f.sync_data()?;
        let meta = lap.metadata();
        self.db.execute(
            "INSERT OR IGNORE INTO laps VALUES(?,?,?,?,?)",
            params![
                lap.id,
                event,
                lap.entry_id,
                serde_json::to_string(&meta)?,
                path
            ],
        )?;
        Ok(())
    }
    pub fn lap(&self, id: &str) -> Result<Lap> {
        let path: String = self
            .db
            .query_row("SELECT path FROM laps WHERE id=?", [id], |r| r.get(0))?;
        let bytes = fs::read(self.root.join(path))?;
        Ok(rmp_serde::from_slice(&decode_limited(&bytes)?)?)
    }
    pub fn lap_metadata(&self, event: &str, entry: &str) -> Result<Vec<Lap>> {
        let mut q = self.db.prepare(
            "SELECT metadata FROM laps WHERE event=? AND entry_id=? ORDER BY rowid DESC LIMIT 1200",
        )?;
        let result = q
            .query_map(params![event, entry], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect();
        result
    }
}
pub fn decode_limited(data: &[u8]) -> Result<Vec<u8>> {
    let d = zstd::stream::read::Decoder::new(data)?;
    let mut raw = Vec::new();
    d.take(16 * 1024 * 1024 + 1).read_to_end(&mut raw)?;
    ensure!(raw.len() <= 16 * 1024 * 1024, "decompressed limit exceeded");
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_deduplicated_restart() {
        let d = tempfile::tempdir().unwrap();
        let b = Batch {
            version: 1,
            event_id: "e".into(),
            entry_id: "c".into(),
            source_id: "../../bad".into(),
            run_id: "r".into(),
            seq: 1,
            historical: false,
            samples: vec![Sample::default()],
        };
        {
            let mut s = Storage::open(d.path()).unwrap();
            assert!(s.append(&b).unwrap());
            assert!(!s.append(&b).unwrap());
        }
        let s = Storage::open(d.path()).unwrap();
        let b2 = s.batches("e", None, 10).unwrap();
        assert_eq!(b2.len(), 1);
        assert_eq!(b2[0].source_id, b.source_id);
    }
    #[test]
    fn truncated_tail_is_not_indexed() {
        let d = tempfile::tempdir().unwrap();
        let s = Storage::open(d.path()).unwrap();
        fs::write(d.path().join("chunks/orphan.zstframes"), [1, 2, 3]).unwrap();
        assert!(s.batches("e", None, 10).unwrap().is_empty());
    }
}
