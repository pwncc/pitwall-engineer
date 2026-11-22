use lmu_pitwall::engineer::{
    analysis::{Entry, calculate_strategy},
    model::*,
    storage::Storage,
};

fn frame(t: f64, driver: &str) -> Sample {
    let lap = (t / 120.) as i32;
    let phase = t % 120.;
    Sample {
        captured_ms: 1_800_000_000_000 + (t * 1000.) as u64,
        game_s: t,
        track: "Monza".into(),
        track_length_m: 5793.,
        session_kind: 10,
        phase: 5,
        driver: driver.into(),
        vehicle: "GT3".into(),
        class: "LMGT3".into(),
        lap,
        lap_distance_m: phase / 120. * 5793.,
        lap_start_s: lap as f64 * 120.,
        last_lap_s: Some(120.),
        authoritative: true,
        valid_lap: true,
        fuel_l: Some(100. - (t / 120. * 3.) % 90.),
        fuel_capacity_l: Some(100.),
        speed_kmh: 180.,
        ..Default::default()
    }
}

#[test]
fn six_hour_replay_preserves_laps_and_bounds_trace_memory() {
    let start = std::time::Instant::now();
    let mut entry = Entry::new("car".into(), "pc-a".into(), frame(0., "Alice"), 1);
    for tick in 1..=216000 {
        let t = tick as f64 / 10.;
        let driver = if t < 10800. { "Alice" } else { "Bob" };
        let mut s = frame(t, driver);
        // Model a complete resource refill inside a pit visit every 30 laps.
        if t % 3600. < 10. && t > 100. {
            s.in_pits = true;
            s.valid_lap = false;
        }
        entry.update(
            if t < 10800. { "pc-a" } else { "pc-b" },
            s,
            1_800_000_000_000 + (t * 1000.) as u64,
        );
    }
    assert_eq!(entry.last.lap, 180);
    assert_eq!(entry.last.driver, "Bob");
    assert!(entry.laps.len() >= 178);
    assert!(entry.drive_times["Alice"] > 10790.);
    assert!(entry.drive_times["Bob"] > 10790.);
    let retained: usize = entry.laps.iter().map(|l| l.samples.len()).sum();
    assert!(retained < 12000, "unbounded trace retention: {retained}");
    let capacity: usize = entry.laps.iter().map(|l| l.samples.capacity()).sum();
    assert!(
        capacity < 20000,
        "old trace allocations retained: {capacity}"
    );
    assert!(
        entry
            .laps
            .iter()
            .filter(|l| l.samples.is_empty())
            .all(|l| l.samples.capacity() == 0)
    );
    let rules = Rules::default();
    let strategy = calculate_strategy(&entry.last, &entry.laps.iter().collect::<Vec<_>>(), &rules);
    assert_eq!(strategy.pace_s, Some(120.));
    assert!((strategy.fuel_per_lap.unwrap() - 3.).abs() < 0.01);
    println!(
        "six-hour replay: {} laps; {} retained trace samples; {:.2}s execution",
        entry.laps.len(),
        retained,
        start.elapsed().as_secs_f64()
    );
}

#[test]
fn gradual_refuelling_excludes_service_lap_and_retains_clean_consumption() {
    let mut e = Entry::new("car".into(), "pc".into(), frame(0., "A"), 1);
    for tick in 1..=6000 {
        let t = tick as f64 / 10.;
        let mut s = frame(t, "A");
        if (250. ..270.).contains(&t) {
            s.in_pits = true;
            s.fuel_l = Some(20. + (t - 250.) * 0.2);
        }
        e.update("pc", s, tick);
    }
    assert!(e.laps.iter().any(|l| !l.valid));
    assert!(
        e.laps
            .iter()
            .filter(|l| l.valid)
            .all(|l| l.fuel_used_l.is_none_or(|v| v >= 0.))
    );
}

#[test]
fn archive_handles_duplicate_packets_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let b = Batch {
        version: 1,
        event_id: "event".into(),
        entry_id: "car".into(),
        source_id: "source".into(),
        run_id: "run".into(),
        seq: 1,
        historical: false,
        samples: (0..50).map(|n| frame(n as f64 / 50., "A")).collect(),
    };
    {
        let mut storage = Storage::open(dir.path()).unwrap();
        assert!(storage.append(&b).unwrap());
        assert!(!storage.append(&b).unwrap());
    }
    let storage = Storage::open(dir.path()).unwrap();
    let b2 = storage.batches("event", Some("car"), 10).unwrap();
    assert_eq!(b2.len(), 1);
    assert_eq!(b2[0].samples.len(), 50);
}

#[test]
fn rejects_overlapping_driver_schedule() {
    let mut r = Rules::default();
    r.driver_plan = vec![
        DriverPlan {
            entry_id: "car".into(),
            driver: "A".into(),
            start_s: 0.,
            end_s: 100.,
        },
        DriverPlan {
            entry_id: "car".into(),
            driver: "B".into(),
            start_s: 90.,
            end_s: 200.,
        },
    ];
    assert!(!r.validate());
}

#[test]
fn confirmed_inventory_and_driver_limits_constrain_live_scenarios() {
    let mut e = Entry::new("car".into(), "pc".into(), frame(0., "A"), 1);
    for tick in 1..=6000 {
        e.update("pc", frame(tick as f64 / 10., "A"), tick);
    }
    let mut rules = Rules::default();
    rules.car_plans.insert(
        "car".into(),
        CarPlan {
            tyres_used: Some(20),
            tyre_allocation: Some(20),
            ..Default::default()
        },
    );
    rules.max_drive_s = Some(660.);
    let view = e.view(&rules, 6000);
    assert!(view.strategy.clean_laps >= 3);
    assert!(
        !view
            .strategy
            .scenarios
            .iter()
            .find(|s| s.name == "Change tyres")
            .unwrap()
            .feasible
    );
    assert!(
        !view
            .strategy
            .scenarios
            .iter()
            .find(|s| s.name == "Extend one lap")
            .unwrap()
            .feasible
    );
    assert!(
        view.advice
            .iter()
            .any(|a| a.id.ends_with(":tyre_inventory"))
    );
    let stale = e.view(&rules, 10000);
    assert!(stale.strategy.scenarios.is_empty());
    assert!(stale.strategy.fuel_add_l.is_none());
}
