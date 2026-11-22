//! Full-volume synthetic archive/restart benchmark; never connects to a game.
use anyhow::{Result, ensure};
use lmu_pitwall::engineer::{model::*, server::App};
use std::{fs, io::Write, path::Path, time::Instant};

fn frame(t: f64, car: usize, scoring: bool, start: u64) -> Sample {
    let lap = (t / 120.) as i32;
    let fraction = t % 120. / 120.;
    let driver = if t < 10800. { "A" } else { "B" };
    Sample {
        captured_ms: start + (t * 1000.) as u64,
        game_s: t,
        track: "Monza (SYNTHETIC ARCHIVE BENCHMARK)".into(),
        vehicle: "GT3 (SYNTHETIC)".into(),
        driver: format!("Driver {car} {driver}"),
        class: "LMGT3".into(),
        slot: Some(car as i32 + 1),
        track_length_m: 5793.,
        session_kind: 10,
        phase: 5,
        session_remaining_s: Some((21600. - t).max(0.)),
        lap,
        lap_distance_m: fraction * 5793.,
        lap_start_s: lap as f64 * 120.,
        last_lap_s: Some(120.),
        authoritative: true,
        valid_lap: true,
        in_pits: t > 100. && t % 3600. < 5.,
        fuel_l: Some(100. - (t / 120. * 3.) % 90.),
        fuel_capacity_l: Some(100.),
        ve: Some(1. - (t / 120. * 0.08) % 0.9),
        soc: Some(0.65),
        water_c: Some(90.),
        oil_c: Some(100.),
        speed_kmh: 180. + 80. * (fraction * std::f64::consts::TAU).sin(),
        throttle: 0.8,
        brake: 0.1,
        steering: (fraction * std::f64::consts::TAU).sin() * 0.3,
        rpm: 6500.,
        gear: 4,
        x: (fraction * std::f64::consts::TAU).sin() * 600.,
        z: (fraction * std::f64::consts::TAU).cos() * 1000.,
        air_c: 24.,
        track_c: 31.,
        wheels: std::array::from_fn(|_| Wheel {
            temperature_c: Some([90., 88., 85.]),
            carcass_c: Some(85.),
            pressure_kpa: Some(160.),
            remaining: Some(1. - (t % 3000.) / 10000.),
            brake_c: Some(500.),
            ..Default::default()
        }),
        standings: if scoring {
            (0..38)
                .map(|i| Standing {
                    slot: i + 1,
                    driver: format!("Synthetic rival {i}"),
                    vehicle: "GT3 (SYNTHETIC)".into(),
                    class: "LMGT3".into(),
                    position: i as u8 + 1,
                    laps: lap,
                    distance_m: (fraction * 5793. - i as f64 * 90.).rem_euclid(5793.),
                    last_lap_s: Some(120.),
                    gap_s: Some(i as f64 * 2.),
                    ..Default::default()
                })
                .collect()
        } else {
            vec![]
        },
        ..Default::default()
    }
}
fn bytes(root: &Path) -> u64 {
    [root.to_path_buf(), root.join("chunks"), root.join("laps")]
        .iter()
        .filter_map(|p| fs::read_dir(p).ok())
        .flatten()
        .filter_map(|e| e.ok()?.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .expect("Provide a fresh benchmark directory");
    let root = Path::new(&root);
    ensure!(
        !root.exists(),
        "Use a fresh directory; existing data is never replaced"
    );
    let app = App::open(root.into())?;
    let started = Instant::now();
    let capture_start = now_ms() - 21600000;
    let mut progress = fs::File::create(root.join("benchmark-progress.jsonl"))?;
    {
        let mut app = app.lock().unwrap();
        let event = app.event_id.clone();
        for second in 0..21600 {
            for car in 0..8 {
                let samples = (0..50)
                    .map(|tick| {
                        let t = if second == 21599 && tick == 49 {
                            21600.
                        } else {
                            second as f64 + tick as f64 / 50.
                        };
                        frame(t, car, tick % 10 == 0, capture_start)
                    })
                    .collect();
                app.ingest(&Batch {
                    version: PROTOCOL,
                    event_id: event.clone(),
                    entry_id: format!("Benchmark car {car}"),
                    source_id: format!(
                        "benchmark-{car}-{}",
                        if second < 10800 { "a" } else { "b" }
                    ),
                    run_id: "full-six-hour-archive".into(),
                    seq: second as u64 + 1,
                    historical: false,
                    samples,
                })?;
            }
            if second % 300 == 0 {
                let size = bytes(root);
                ensure!(
                    size < 3 * 1024 * 1024 * 1024,
                    "Archive reached the 3 GiB benchmark disk budget"
                );
                writeln!(
                    progress,
                    "{}",
                    serde_json::json!({"simulation_s":second,"elapsed_s":started.elapsed().as_secs_f64(),"bytes":size,"batches":app.live_batches})
                )?;
                progress.flush()?;
            }
        }
        ensure!(app.entries.len() == 8 && app.live_batches == 172800);
        for e in app.entries.values() {
            ensure!(e.last.lap == 180 && e.last.driver.ends_with('B'));
            ensure!(e.drive_times.values().all(|t| *t > 10790.));
            ensure!(e.stint_start == 10800.);
            ensure!(e.laps.iter().map(|l| l.samples.capacity()).sum::<usize>() < 20000);
        }
    }
    let write_s = started.elapsed().as_secs_f64();
    drop(app);
    let restart = Instant::now();
    let restored = App::open(root.into())?;
    let restart_s = restart.elapsed().as_secs_f64();
    let app = restored.lock().unwrap();
    ensure!(
        app.errors.is_empty(),
        "Restore reported errors: {:?}",
        app.errors
    );
    ensure!(app.entries.len() == 8);
    for e in app.entries.values() {
        ensure!(e.last.lap == 180 && e.received == 0);
        ensure!(e.drive_times.values().all(|t| *t > 10790.));
    }
    let report = serde_json::json!({"samples":8640000,"batches":172800,"cars":8,"hours":6,"acquisition_hz":50,"scoring_cars":38,"scoring_hz":5,"write_s":write_s,"restart_s":restart_s,"bytes":bytes(root),"laps":app.entries.values().map(|e|e.laps.len()).collect::<Vec<_>>(),"errors":app.errors});
    fs::write(
        root.join("benchmark-result.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
