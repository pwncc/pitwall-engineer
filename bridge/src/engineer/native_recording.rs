//! Read-only import of completed native LMU recordings through DuckDB's C ABI.
//! The library stays loaded until every query and connection has been destroyed.
use super::model::*;
use anyhow::{ensure, Context, Result};
use libloading::Library;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_void, CStr, CString},
    path::{Path, PathBuf},
};
type Handle = *mut c_void;
#[repr(C)]
struct RawResult {
    words: [usize; 6],
}
const _: () = assert!(std::mem::size_of::<RawResult>() == 48);
type Open = unsafe extern "C" fn(*const c_char, *mut Handle, Handle, *mut *mut c_char) -> u32;
type Connect = unsafe extern "C" fn(Handle, *mut Handle) -> u32;
type Close = unsafe extern "C" fn(*mut Handle);
type Query = unsafe extern "C" fn(Handle, *const c_char, *mut RawResult) -> u32;
type Destroy = unsafe extern "C" fn(*mut RawResult);
type Count = unsafe extern "C" fn(*mut RawResult) -> u64;
type Value = unsafe extern "C" fn(*mut RawResult, u64, u64) -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_void);
type Error = unsafe extern "C" fn(*mut RawResult) -> *const c_char;
struct Duck {
    lib: Library,
    db: Handle,
    conn: Handle,
    query: Query,
    destroy: Destroy,
    rows: Count,
    columns: Count,
    value: Value,
    free: Free,
    error: Error,
    disconnect: Close,
    close: Close,
}
impl Duck {
    fn open(path: &Path) -> Result<Self> {
        Self::open_mode(path, true)
    }
    fn open_mode(path: &Path, read_only: bool) -> Result<Self> {
        let mut candidates = vec![std::env::current_exe()?
            .parent()
            .context("EXE directory")?
            .join("duckdb.dll")];
        if let Some(path) = std::env::var_os("LMU_DUCKDB_DLL") {
            candidates.insert(0, PathBuf::from(path));
        }
        candidates.push(PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\Le Mans Ultimate\duckdb.dll",
        ));
        let dll=candidates.into_iter().find(|p|p.is_absolute()&&p.is_file()).context("duckdb.dll is unavailable. Use the complete engineer package, or set LMU_DUCKDB_DLL to the installed game's DLL")?;
        // Only an absolute, locally installed library path is used.
        unsafe {
            let lib = Library::new(&dll).context("Cannot load the native recording runtime. Keep duckdb.dll in the complete release folder and install the Microsoft Visual C++ v14 x64 Redistributable if its runtime is missing")?;
            let create: unsafe extern "C" fn(*mut Handle) -> u32 =
                *lib.get(b"duckdb_create_config\0")?;
            let set: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> u32 =
                *lib.get(b"duckdb_set_config\0")?;
            let destroy_config: Close = *lib.get(b"duckdb_destroy_config\0")?;
            let open: Open = *lib.get(b"duckdb_open_ext\0")?;
            let connect: Connect = *lib.get(b"duckdb_connect\0")?;
            let close: Close = *lib.get(b"duckdb_close\0")?;
            let disconnect: Close = *lib.get(b"duckdb_disconnect\0")?;
            let free: Free = *lib.get(b"duckdb_free\0")?;
            let mut config = std::ptr::null_mut();
            ensure!(create(&mut config) == 0, "DuckDB configuration failed");
            for (key, value) in [
                (
                    "access_mode",
                    if read_only { "READ_ONLY" } else { "READ_WRITE" },
                ),
                ("memory_limit", "512MB"),
                ("threads", "2"),
                ("enable_external_access", "false"),
                ("autoload_known_extensions", "false"),
                ("autoinstall_known_extensions", "false"),
            ] {
                let key = CString::new(key)?;
                let value = CString::new(value)?;
                if set(config, key.as_ptr(), value.as_ptr()) != 0 {
                    destroy_config(&mut config);
                    anyhow::bail!("DuckDB cannot enforce read-only import configuration");
                }
            }
            let filename = CString::new(
                path.to_str()
                    .context("Recording path must be valid Unicode")?,
            )?;
            let mut db = std::ptr::null_mut();
            let mut err = std::ptr::null_mut();
            let state = open(filename.as_ptr(), &mut db, config, &mut err);
            destroy_config(&mut config);
            if state != 0 {
                let message = if err.is_null() {
                    "Could not open completed recording".into()
                } else {
                    let s = CStr::from_ptr(err).to_string_lossy().into_owned();
                    free(err as _);
                    s
                };
                anyhow::bail!(message);
            }
            let mut conn = std::ptr::null_mut();
            if connect(db, &mut conn) != 0 {
                close(&mut db);
                anyhow::bail!("DuckDB connection failed");
            }
            // Resolve before constructing so each result uses the same C ABI as its owner.
            let result = Self {
                query: *lib.get(b"duckdb_query\0")?,
                destroy: *lib.get(b"duckdb_destroy_result\0")?,
                rows: *lib.get(b"duckdb_row_count\0")?,
                columns: *lib.get(b"duckdb_column_count\0")?,
                value: *lib.get(b"duckdb_value_varchar\0")?,
                error: *lib.get(b"duckdb_result_error\0")?,
                lib,
                db,
                conn,
                free,
                disconnect,
                close,
            };
            Ok(result)
        }
    }
    fn query(&self, sql: &str, limit: usize) -> Result<Vec<Vec<String>>> {
        unsafe {
            let sql = CString::new(sql)?;
            let mut raw = RawResult { words: [0; 6] };
            let status = (self.query)(self.conn, sql.as_ptr(), &mut raw);
            struct Guard<'a> {
                raw: &'a mut RawResult,
                duck: &'a Duck,
            }
            impl Drop for Guard<'_> {
                fn drop(&mut self) {
                    unsafe {
                        (self.duck.destroy)(self.raw);
                    }
                }
            }
            let guard = Guard {
                raw: &mut raw,
                duck: self,
            };
            if status != 0 {
                let ptr = (self.error)(guard.raw);
                let message = if ptr.is_null() {
                    "Native query failed".into()
                } else {
                    CStr::from_ptr(ptr).to_string_lossy().into_owned()
                };
                anyhow::bail!(message);
            }
            let rows = (self.rows)(guard.raw) as usize;
            let cols = (self.columns)(guard.raw) as usize;
            ensure!(
                rows <= limit && cols <= 128,
                "Native recording exceeds bounded query limit"
            );
            let mut output = Vec::with_capacity(rows);
            for row in 0..rows {
                let mut cells = Vec::with_capacity(cols);
                for col in 0..cols {
                    let ptr = (self.value)(guard.raw, col as u64, row as u64);
                    let cell = if ptr.is_null() {
                        String::new()
                    } else {
                        let value = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                        (self.free)(ptr as _);
                        value
                    };
                    ensure!(cell.len() <= 1024 * 1024, "Native cell exceeds limit");
                    cells.push(cell);
                }
                output.push(cells);
            }
            Ok(output)
        }
    }
    fn scalar(&self, sql: &str) -> Result<f64> {
        self.query(sql, 1)?
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .context("Missing numeric native recording value")
    }
}
impl Drop for Duck {
    fn drop(&mut self) {
        unsafe {
            (self.disconnect)(&mut self.conn);
            (self.close)(&mut self.db);
        }
        let _ = &self.lib;
    }
}
fn ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn recording(path: &str) -> Result<PathBuf> {
    let path = std::fs::canonicalize(path)?;
    ensure!(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("duckdb")),
        "Select a completed .duckdb recording"
    );
    let meta = std::fs::metadata(&path)?;
    ensure!(
        meta.is_file() && meta.len() <= 8 * 1024 * 1024 * 1024,
        "Recording must be a regular file under 8 GiB"
    );
    Ok(path)
}
#[derive(Serialize, Deserialize, Debug)]
pub struct NativeLap {
    pub start_s: f64,
    pub end_s: f64,
    pub time_s: f64,
}
#[derive(Serialize)]
pub struct Inspection {
    pub metadata: BTreeMap<String, String>,
    pub laps: Vec<NativeLap>,
    pub channels: Vec<String>,
    pub channel_descriptors: Vec<Vec<String>>,
    pub gps_start: f64,
    pub gps_end: f64,
    pub unit_hints: BTreeMap<String, String>,
    pub speed_channel: String,
    pub warnings: Vec<String>,
}
fn inspect_db(db: &Duck) -> Result<Inspection> {
    let channels = db
        .query("SHOW TABLES", 4096)?
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect::<Vec<_>>();
    let metadata = db
        .query("SELECT key,value FROM metadata LIMIT 1000", 1000)?
        .into_iter()
        .filter(|r| r.len() == 2)
        .map(|r| (r[0].clone(), r[1].clone()))
        .collect();
    let gps_start = db.scalar("SELECT min(value) FROM \"GPS Time\"")?;
    let gps_end = db.scalar("SELECT max(value) FROM \"GPS Time\"")?;
    ensure!(gps_end > gps_start, "No complete recording timeline");
    let laps=db.query("SELECT ts,value FROM \"Lap Time\" WHERE value>20 AND value<1800 ORDER BY ts LIMIT 2000",2000)?.into_iter().filter_map(|r|{let end=r.first()?.parse::<f64>().ok()?;let time=r.get(1)?.parse::<f64>().ok()?;let start=end-time;(start>=gps_start-0.1&&end<=gps_end+0.1).then_some(NativeLap{start_s:start,end_s:end,time_s:time})}).collect();
    let channel_descriptors = if channels.iter().any(|t| t == "channelsList") {
        db.query("SELECT * FROM channelsList LIMIT 256", 256)?
    } else {
        vec![]
    };
    let unit_hints = channel_descriptors
        .iter()
        .filter(|row| row.len() >= 3)
        .map(|row| (row[0].clone(), row[2].clone()))
        .collect();
    let speed_channel = if channels.iter().any(|c| c == "Ground Speed") {
        "Ground Speed"
    } else {
        "GPS Speed"
    }
    .into();
    Ok(Inspection{metadata,laps,channels,channel_descriptors,gps_start,gps_end,unit_hints,speed_channel,warnings:vec!["Only completed laps fully covered by GPS Time are offered".into(),"Continuous channels without timestamps are aligned over the recorded GPS-time extent. Imported references are descriptive and excluded from automatic advice and consumption learning".into(),"Confirm speed and input units against the channel descriptors before importing".into()]})
}
pub fn inspect(path: &str) -> Result<Inspection> {
    let path = recording(path)?;
    let mut info = inspect_db(&Duck::open(&path)?)?;
    if path.with_extension("duckdb.wal").exists() {
        info.warnings.push("A companion WAL file is present. Keep it beside the recording when copying; it may contain the most recent telemetry.".into());
    }
    Ok(info)
}
#[derive(Deserialize)]
pub struct ImportRequest {
    pub path: String,
    pub entry_id: String,
    pub start_s: f64,
    pub end_s: f64,
    pub speed_unit: String,
    pub input_scale: f64,
}
struct Channel(Vec<(f64, f64)>);
fn distance_window(distance: &Channel, start: f64, end: f64) -> Result<(f64, f64)> {
    let (mut first, mut last) = (start, end);
    for pair in distance.0.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a.1 <= b.1 + 10. {
            continue;
        }
        // Sampled distance and lap events need not land on the same physics tick.
        // Trim the straddling interval rather than interpolating across its reset.
        let tolerance = (b.0 - a.0).abs().clamp(0.1, 1.) + 0.05;
        if (a.0 - start).abs().min((b.0 - start).abs()) <= tolerance {
            first = first.max(b.0);
        } else if (a.0 - end).abs().min((b.0 - end).abs()) <= tolerance {
            last = last.min(a.0);
        } else if b.0 > start && a.0 < end {
            anyhow::bail!("Distance resets inside selected lap; choose an uninterrupted lap");
        }
    }
    ensure!(
        last > first && first - start <= 1.1 && end - last <= 1.1,
        "Insufficient continuous distance coverage at lap boundaries"
    );
    Ok((first, last))
}
impl Channel {
    fn at(&self, t: f64) -> Option<f64> {
        let i = self.0.partition_point(|(time, _)| *time < t);
        if let Some((time, value)) = self.0.get(i).filter(|(time, _)| *time == t) {
            let _ = time;
            return Some(*value);
        }
        if i == 0 {
            return self.0.first().map(|(_, v)| *v);
        }
        if i >= self.0.len() {
            return self.0.last().map(|(_, v)| *v);
        }
        let (a, v) = self.0[i - 1];
        let (b, w) = self.0[i];
        Some(if b > a {
            v + (w - v) * (t - a) / (b - a)
        } else {
            v
        })
    }
}
fn channel(db: &Duck, name: &str, info: &Inspection, start: f64, end: f64) -> Result<Channel> {
    ensure!(
        info.channels.iter().any(|s| s == name),
        "Channel missing: {name}"
    );
    let table = ident(name);
    let columns = db.query(&format!("DESCRIBE {table}"), 128)?;
    let timestamped = columns.iter().any(|r| r.first().is_some_and(|s| s == "ts"));
    ensure!(
        columns
            .iter()
            .any(|r| r.first().is_some_and(|s| s == "value")),
        "Unsupported scalar channel: {name}"
    );
    let rows = if timestamped {
        db.query(
            &format!(
                "SELECT ts,value FROM {table} WHERE ts>={} AND ts<={} ORDER BY ts LIMIT 20001",
                start, end
            ),
            20000,
        )?
    } else {
        let count = db.scalar(&format!("SELECT count(*) FROM {table}"))?;
        ensure!(count > 1., "Empty channel: {name}");
        let first = (((start - info.gps_start) / (info.gps_end - info.gps_start) * (count - 1.))
            .floor()
            - 2.)
            .max(0.) as u64;
        let last =
            (((end - info.gps_start) / (info.gps_end - info.gps_start) * (count - 1.)).ceil() + 2.)
                .min(count - 1.) as u64;
        let step = ((last - first + 19000) / 19000).max(1);
        let mut rows=db.query(&format!("SELECT rowid,value FROM {table} WHERE rowid BETWEEN {first} AND {last} AND rowid%{step}=0 ORDER BY rowid LIMIT 20001"),20000)?;
        for row in &mut rows {
            let index = row[0].parse::<f64>()?;
            row[0] = (info.gps_start + index / (count - 1.) * (info.gps_end - info.gps_start))
                .to_string();
        }
        rows
    };
    let points = rows
        .into_iter()
        .filter_map(|r| {
            let t = r.first()?.parse::<f64>().ok()?;
            let v = r.get(1)?.parse::<f64>().ok()?;
            (t.is_finite() && v.is_finite()).then_some((t, v))
        })
        .collect::<Vec<_>>();
    ensure!(points.len() > 1, "Insufficient {name} coverage");
    Ok(Channel(points))
}
pub fn import(request: &ImportRequest) -> Result<Lap> {
    ensure!(
        request.start_s.is_finite()
            && request.end_s.is_finite()
            && request.end_s - request.start_s > 20.
            && request.end_s - request.start_s < 1800.,
        "Choose a complete bounded lap"
    );
    ensure!(
        ["m/s", "km/h"].contains(&request.speed_unit.as_str()),
        "Confirm speed units"
    );
    ensure!(
        [1., 100.].contains(&request.input_scale),
        "Input scale must be 1 (fractions) or 100 (percent)"
    );
    let path = recording(&request.path)?;
    let db = Duck::open(&path)?;
    let info = inspect_db(&db)?;
    ensure!(
        info.laps
            .iter()
            .any(|l| (l.start_s - request.start_s).abs() < 0.01
                && (l.end_s - request.end_s).abs() < 0.01),
        "Lap boundary does not match this recording"
    );
    let start = request.start_s;
    let end = request.end_s;
    let load = |name: &str| channel(&db, name, &info, start, end);
    let dist = load("Lap Dist")?;
    let (sample_start, sample_end) = distance_window(&dist, start, end)?;
    let speed = load(&info.speed_channel)?;
    let throttle = load("Throttle Pos")?;
    let brake = load("Brake Pos")?;
    let steering = load("Steering Pos")?;
    if let Some(unit) = info.unit_hints.get(&info.speed_channel) {
        ensure!(
            unit == &request.speed_unit,
            "Selected speed units do not match the recorded speed channel descriptor"
        );
    }
    let fuel = if info.unit_hints.get("Fuel Level").is_some_and(|u| u == "L") {
        load("Fuel Level").ok()
    } else {
        None
    };
    let ve = if info
        .unit_hints
        .get("Virtual Energy")
        .is_some_and(|u| ["%", "fraction", "ratio"].contains(&u.as_str()))
    {
        load("Virtual Energy").ok()
    } else {
        None
    };
    let meta = |key: &str| {
        info.metadata
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("Unknown {key}"))
    };
    let count = ((sample_end - sample_start) * 10.).floor() as usize + 1;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let t = sample_start + i as f64 / 10.;
        let s = Sample {
            captured_ms: (t * 1000.).max(0.) as u64,
            game_s: t,
            lap_start_s: start,
            track: meta("TrackName"),
            vehicle: meta("CarName"),
            driver: meta("DriverName"),
            class: meta("CarClass"),
            lap: 0,
            lap_distance_m: dist.at(t).context("Distance missing")?,
            speed_kmh: speed.at(t).unwrap_or(0.)
                * if request.speed_unit == "m/s" { 3.6 } else { 1. },
            throttle: throttle.at(t).unwrap_or(0.) / request.input_scale,
            brake: brake.at(t).unwrap_or(0.) / request.input_scale,
            steering: steering.at(t).unwrap_or(0.) / request.input_scale,
            fuel_l: fuel.as_ref().and_then(|c| c.at(t)),
            ve: ve
                .as_ref()
                .and_then(|c| c.at(t))
                .map(|v| {
                    if info
                        .unit_hints
                        .get("Virtual Energy")
                        .is_some_and(|u| u == "%")
                    {
                        v / 100.
                    } else {
                        v
                    }
                })
                .filter(|v| (0. ..=1.).contains(v)),
            authoritative: false,
            valid_lap: false,
            supplemental: Some(
                serde_json::json!({"native_import":true,"position_available":false}),
            ),
            ..Default::default()
        };
        ensure!(
            s.validate()
                && (-0.001..=1.001).contains(&s.throttle)
                && (-0.001..=1.001).contains(&s.brake)
                && s.steering.abs() <= 1.01,
            "Imported channels failed validation; check speed and input units"
        );
        samples.push(s);
    }
    ensure!(
        samples
            .windows(2)
            .all(|w| w[1].lap_distance_m + 10. >= w[0].lap_distance_m),
        "Distance wraps within selected lap; select an unambiguous complete lap"
    );
    let length = samples.iter().map(|s| s.lap_distance_m).fold(0., f64::max);
    for s in &mut samples {
        s.track_length_m = length;
    }
    let first = samples.first().unwrap();
    Ok(Lap{id:new_id(),entry_id:request.entry_id.clone(),driver:first.driver.clone(),vehicle:first.vehicle.clone(),track:first.track.clone(),lap:0,time_s:end-start,fuel_used_l:None,ve_used:None,valid:false,reference_only:true,reason:"Native import: descriptive reference; channels aligned over recording extent. Verify units, coverage and conditions.".into(),fuel_start_l:first.fuel_l,rain:0.,compound:255,samples})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lap_distance_resets_are_trimmed_without_interpolating_across_finish() {
        let distance = Channel(vec![
            (99.9, 5790.),
            (100., 5792.),
            (100.1, 2.),
            (209.9, 5788.),
            (210., 5792.),
            (210.1, 1.),
        ]);
        let (start, end) = distance_window(&distance, 100.04, 210.04).unwrap();
        assert_eq!((start, end), (100.1, 210.));
        assert_eq!(distance.at(start), Some(2.));
        assert_eq!(distance.at(end), Some(5792.));
        let interrupted = Channel(vec![(100., 1.), (150., 2000.), (150.1, 0.), (210., 5700.)]);
        assert!(distance_window(&interrupted, 100., 210.).is_err());
    }
    #[test]
    fn channel_interpolation() {
        let c = Channel(vec![(10., 2.), (20., 4.)]);
        assert_eq!(c.at(15.), Some(3.));
    }
    #[test]
    fn identifiers_are_quoted() {
        assert_eq!(ident("a\"b"), "\"a\"\"b\"");
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn reads_native_schema_without_mutating_recording() {
        let path = tempfile::tempdir().unwrap();
        let file = path.path().join("fixture.duckdb");
        let db=Duck::open_mode(&file,false).expect("Provide LMU_DUCKDB_DLL or the packaged DuckDB DLL for native import integration testing");
        db.query("CREATE TABLE metadata(key VARCHAR,value VARCHAR);INSERT INTO metadata VALUES ('TrackName','Monza'),('CarName','GT3'),('DriverName','Alice'),('CarClass','LMGT3');CREATE TABLE \"GPS Time\" AS SELECT i*0.01 AS value FROM range(24001) t(i);CREATE TABLE \"Lap Time\" AS SELECT 120.0 AS ts,120.0 AS value UNION ALL SELECT 240.0,120.0",100).unwrap();
        for (name, expr) in [
            ("Lap Dist", "(i*0.01%120)/120*5793"),
            ("Ground Speed", "160.0/3.6"),
            ("Throttle Pos", "0.8"),
            ("Brake Pos", "0.1"),
            ("Steering Pos", "0.05"),
            ("Fuel Level", "100.0-i*0.01/40"),
            ("Virtual Energy", "80.0"),
        ] {
            db.query(
                &format!(
                    "CREATE TABLE {} AS SELECT {expr} AS value FROM range(24001) t(i)",
                    ident(name)
                ),
                100,
            )
            .unwrap();
        }
        db.query("CREATE TABLE channelsList(channel VARCHAR,frequency INTEGER,unit VARCHAR);INSERT INTO channelsList VALUES ('Ground Speed',100,'m/s'),('Fuel Level',20,'L'),('Virtual Energy',20,'%')",100).unwrap();
        drop(db);
        let before = std::fs::read(&file).unwrap();
        let info = inspect(file.to_str().unwrap()).unwrap();
        assert_eq!(info.laps.len(), 2);
        let lap = import(&ImportRequest {
            path: file.to_string_lossy().into(),
            entry_id: "car".into(),
            start_s: 0.,
            end_s: 120.,
            speed_unit: "m/s".into(),
            input_scale: 1.,
        })
        .unwrap();
        assert_eq!(lap.samples.len(), 1200);
        assert!((lap.samples[500].speed_kmh - 160.).abs() < 0.01);
        assert!(lap.reference_only && !lap.valid);
        assert_eq!(lap.samples[500].ve, Some(0.8));
        assert!(lap.samples[500].fuel_l.is_some());
        assert!(import(&ImportRequest {
            path: file.to_string_lossy().into(),
            entry_id: "car".into(),
            start_s: 0.,
            end_s: 120.,
            speed_unit: "km/h".into(),
            input_scale: 1.
        })
        .is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
}
