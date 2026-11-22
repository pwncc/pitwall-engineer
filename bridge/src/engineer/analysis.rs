use super::model::*;
use std::collections::{BTreeMap, VecDeque};

pub fn median(mut v: Vec<f64>) -> Option<f64> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 0 {
        (v[n / 2 - 1] + v[n / 2]) / 2.
    } else {
        v[n / 2]
    })
}
fn diff(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    a.zip(b)
        .map(|(a, b)| a - b)
        .filter(|v| v.is_finite() && *v >= 0.)
}
pub struct Entry {
    pub id: String,
    pub source: String,
    pub received: u64,
    pub last: Sample,
    pub laps: VecDeque<Lap>,
    current: Vec<Sample>,
    current_bad: bool,
    current_partial: bool,
    fuel_start: Option<f64>,
    ve_start: Option<f64>,
    pub drive_times: BTreeMap<String, f64>,
    pub stint_start: f64,
    pub conflict: bool,
}
impl Entry {
    pub fn new(id: String, source: String, s: Sample, received: u64) -> Self {
        Self {
            id,
            source,
            received,
            stint_start: s.game_s,
            current_partial: s.lap_distance_m > 80.,
            fuel_start: s.fuel_l,
            ve_start: s.ve,
            current: vec![compact(&s)],
            current_bad: !s.valid_lap || s.in_pits || s.yellow,
            laps: VecDeque::new(),
            drive_times: BTreeMap::new(),
            last: s,
            conflict: false,
        }
    }
    pub fn update(&mut self, source: &str, mut s: Sample, received: u64) -> Vec<RaceEvent> {
        let mut events = vec![];
        let dt = s.game_s - self.last.game_s;
        // Late packets from the old driving seat must not reverse a handover
        // or turn its older game clock into a session reset while the new feed is fresh.
        if self.source != source
            && self.received.saturating_add(3000) > received
            && self.last.authoritative
            && s.authoritative
            && s.track == self.last.track
            && s.session_kind == self.last.session_kind
            && s.game_s <= self.last.game_s
        {
            self.conflict = true;
            return events;
        }
        // A second active PC may still publish the old seat briefly. Do not combine its samples.
        if self.source != source
            && self.received + 3000 > received
            && self.last.authoritative
            && s.authoritative
            && (self.last.driver == s.driver
                || (self.last.vehicle != s.vehicle && self.last.slot != s.slot))
        {
            self.conflict = true;
            return events;
        }
        self.conflict = false;
        if !s.authoritative && self.last.authoritative && self.received + 3000 > received {
            return events;
        }
        let reset = dt < -1.
            || s.track != self.last.track
            || s.session_kind != self.last.session_kind
            || s.lap < self.last.lap;
        if !reset {
            if s.standings.is_empty() {
                s.standings = std::mem::take(&mut self.last.standings);
            }
            if s.supplemental.is_none()
                && s.authoritative
                && self.last.authoritative
                && self
                    .last
                    .supplemental
                    .as_ref()
                    .and_then(|v| v.get("observed_ms"))
                    .and_then(|v| v.as_u64())
                    .is_some_and(|at| s.captured_ms.saturating_sub(at) <= 15000)
            {
                s.supplemental = self.last.supplemental.take();
            }
        }
        let swap = s.driver != self.last.driver && s.authoritative;
        let refuel = s
            .fuel_l
            .zip(self.last.fuel_l)
            .is_some_and(|(a, b)| a > b + 0.01)
            || s.ve.zip(self.last.ve).is_some_and(|(a, b)| a > b + 0.0001);
        if reset {
            events.push(self.event(&s, "session", "Session changed or restarted"));
            self.current.clear();
            self.laps.clear();
            self.drive_times.clear();
            self.current_partial = true;
            self.stint_start = s.game_s;
        }
        if !self.last.authoritative && s.authoritative && self.drive_times.is_empty() {
            self.stint_start = s.game_s;
        }
        if swap {
            events.push(self.event(
                &s,
                "driver",
                &format!("{} → {}", self.last.driver, s.driver),
            ));
            self.current.clear();
            self.current_partial = true;
            self.stint_start = s.game_s;
        }
        if dt > 0. && dt < 2. && s.authoritative && self.last.authoritative && !swap {
            *self.drive_times.entry(s.driver.clone()).or_default() += dt;
        }
        if s.in_pits && !self.last.in_pits {
            events.push(self.event(&s, "pit_in", "Entered pit lane"));
        }
        if !s.in_pits && self.last.in_pits {
            events.push(self.event(&s, "pit_out", "Left pit lane"));
        }
        if refuel {
            self.current_bad = true;
        }
        if self.last.in_pits
            && !s.in_pits
            && (diff(s.fuel_l, self.fuel_start).is_some_and(|v| v > 0.1)
                || diff(s.ve, self.ve_start).is_some_and(|v| v > 0.001))
        {
            events.push(self.event(&s, "refuel", "Resources replenished during stop"));
        }
        if s.wheels.iter().zip(self.last.wheels.iter()).any(|(a, b)| {
            a.remaining
                .zip(b.remaining)
                .is_some_and(|(a, b)| a > b + 0.05)
        }) {
            events.push(self.event(
                &s,
                "tyres",
                "Tyre remaining life increased; check tyre change",
            ));
        }
        if s.impact_s > self.last.impact_s && s.impact > 0. {
            events.push(self.event(&s, "impact", &format!("Impact recorded: {:.0}", s.impact)));
        }
        // A missing interval can straddle the finish line. Reject the completed
        // lap before finalising it as well as the new lap below.
        if dt > 1. && !reset && !swap {
            self.current_bad = true;
        }
        if s.lap > self.last.lap && !reset && !swap {
            let t = s
                .last_lap_s
                .unwrap_or(s.lap_start_s - self.last.lap_start_s);
            let complete =
                !self.current_partial && self.current.len() > 10 && s.lap == self.last.lap + 1;
            let fuel = diff(self.fuel_start, s.fuel_l);
            let ve = diff(self.ve_start, s.ve);
            let valid =
                complete && !self.current_bad && t > 20. && t < 1800. && self.last.authoritative;
            let reason = if !complete {
                "Partial lap / missing coverage"
            } else if self.current_bad {
                "Pit, yellow, invalid lap or data gap"
            } else {
                "Clean"
            };
            use sha2::{Digest, Sha256};
            let id = hex::encode(Sha256::digest(
                format!(
                    "{}:{}:{}:{}",
                    self.id,
                    self.source,
                    self.current.first().map(|p| p.captured_ms).unwrap_or(0),
                    self.last.lap
                )
                .as_bytes(),
            ));
            let lap = Lap {
                id,
                entry_id: self.id.clone(),
                driver: self.last.driver.clone(),
                vehicle: self.last.vehicle.clone(),
                track: self.last.track.clone(),
                lap: self.last.lap,
                time_s: t,
                fuel_used_l: fuel.filter(|v| *v > 0. && *v < 100.),
                ve_used: ve.filter(|v| *v > 0. && *v <= 1.),
                valid,
                reference_only: false,
                reason: reason.into(),
                fuel_start_l: self.fuel_start,
                rain: self.last.rain,
                compound: self.last.wheels[0].compound,
                samples: std::mem::take(&mut self.current),
            };
            if t.is_finite() && t > 0. {
                self.laps.push_back(lap);
                // Keep recent traces and the fastest compatible reference in memory.
                // All completed traces are archived by the receiver before the next lap.
                let best = self
                    .laps
                    .iter()
                    .filter(|l| l.valid)
                    .min_by(|a, b| a.time_s.total_cmp(&b.time_s))
                    .map(|l| l.id.clone());
                for old in self.laps.iter_mut().rev().skip(6) {
                    if Some(&old.id) != best.as_ref() {
                        old.samples = Vec::new();
                    }
                }
                if self.laps.len() > 1200 {
                    self.laps.pop_front();
                }
            }
            self.current_bad = false;
            self.current_partial = false;
            self.fuel_start = s.fuel_l;
            self.ve_start = s.ve;
        }
        if self.current.is_empty() {
            self.fuel_start = s.fuel_l;
            self.ve_start = s.ve;
        }
        self.current_bad |= !s.valid_lap || s.in_pits || s.yellow || dt > 1. || !s.authoritative;
        if self
            .current
            .last()
            .is_none_or(|p| s.game_s - p.game_s >= 0.099)
            && self.current.len() < 20000
        {
            self.current.push(compact(&s));
        }
        self.source = source.into();
        self.received = received;
        self.last = s;
        events
    }
    fn event(&self, s: &Sample, kind: &str, text: &str) -> RaceEvent {
        RaceEvent {
            id: {
                use sha2::{Digest, Sha256};
                hex::encode(Sha256::digest(
                    format!(
                        "{}:{}:{}:{}:{}",
                        self.id, self.source, s.captured_ms, kind, text
                    )
                    .as_bytes(),
                ))
            },
            at_ms: s.captured_ms,
            game_s: s.game_s,
            entry_id: self.id.clone(),
            kind: kind.into(),
            text: text.into(),
            manual: false,
        }
    }
    pub fn view(&self, rules: &Rules, now: u64) -> EntryView {
        let mut drive_times = self.drive_times.clone();
        if let Some(adjustments) = rules.drive_time_adjustments.get(&self.id) {
            for (driver, seconds) in adjustments {
                let time = drive_times.entry(driver.clone()).or_default();
                *time = (*time + seconds).max(0.);
            }
        }
        let mut effective = rules.clone();
        let car_plan = rules.car_plans.get(&self.id);
        if let Some(plan) = car_plan {
            plan.apply(&mut effective);
        }
        let rules = &effective;
        let mut strategy =
            calculate_strategy(&self.last, &self.laps.iter().collect::<Vec<_>>(), rules);
        if let Some(plan) = car_plan {
            if let Some((used, allocation)) = plan
                .tyres_used
                .zip(plan.tyre_allocation.or(rules.tyre_allocation))
            {
                for scenario in &mut strategy.scenarios {
                    if ["Change tyres", "Repair and service"].contains(&scenario.name.as_str())
                        && used.saturating_add(scenario.stops.saturating_mul(4)) > allocation
                    {
                        scenario.feasible = false;
                        scenario.reason="Confirmed tyre inventory cannot cover a full set at every modelled stop".into();
                    }
                }
            }
        }
        let stint_left = rules
            .max_stint_s
            .map(|limit| limit - (self.last.game_s - self.stint_start));
        let total_left = rules
            .max_drive_s
            .map(|limit| limit - drive_times.get(&self.last.driver).copied().unwrap_or(0.));
        let driver_left = match (stint_left, total_left) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if let Some(left) = driver_left {
            for scenario in &mut strategy.scenarios {
                if scenario.name == "Extend one lap"
                    && strategy
                        .pace_s
                        .is_some_and(|pace| left < (distance_to_pit(&self.last, rules) + 1.) * pace)
                {
                    scenario.feasible = false;
                    scenario.reason =
                        "Configured current-driver limit does not allow the extra lap and travel to pit entry".into();
                }
                if scenario.name == "Driver handover"
                    && strategy
                        .pace_s
                        .is_some_and(|pace| left < distance_to_pit(&self.last, rules) * pace)
                {
                    scenario.feasible = false;
                    scenario.reason="Configured driver limit expires before the next pit entry; review the observed start time and event rules".into();
                }
            }
        }
        if !rules.driver_plan.is_empty() || driver_left.is_some() {
            strategy.assumptions.push("Driver limits are checked for the next-lap decision. Full-race resource forecasts do not optimise the driver schedule; review handovers separately".into());
        }
        select_preferred(&mut strategy);
        let age = now.saturating_sub(self.received);
        if age > 3000 || !self.last.authoritative {
            strategy.scenarios.clear();
            strategy.fuel_add_l = None;
            strategy.ve_add = None;
            strategy.stops = None;
            strategy.preferred_scenario = None;
            strategy
                .assumptions
                .push("Live strategy suppressed: stale or spectator feed".into());
        }
        let mut advice = vec![];
        let mut push = |id: &str, severity: &str, title: &str, detail: String, n: usize| {
            advice.push(Advice {
                id: format!("{}:{id}", self.id),
                severity: severity.into(),
                title: title.into(),
                detail,
                confidence: if n >= 10 {
                    "high"
                } else if n >= 3 {
                    "medium"
                } else {
                    "low"
                }
                .into(),
                evidence: n,
            })
        };
        if age > 3000 {
            push(
                "stale",
                "critical",
                "Telemetry is stale",
                if self.received == 0 {
                    "History restored; waiting for a live driver connection.".into()
                } else {
                    format!(
                        "Last live receipt {:.1}s ago; confirm connection before calling strategy.",
                        age as f64 / 1000.
                    )
                },
                0,
            )
        }
        if self.conflict {
            push(
                "conflict",
                "critical",
                "Two active feeds disagree",
                "Check which PC controls this entry. Conflicting samples were excluded.".into(),
                0,
            )
        }
        if self.received > 0
            && (self.received as i128 - self.last.captured_ms as i128).abs() > 10000
        {
            push(
                "clock",
                "warning",
                "Source clock or delivery delay needs checking",
                format!(
                    "Receipt and capture clocks differ by {:.1}s. Synchronize the PCs' clocks before relying on cross-PC archive ordering.",
                    (self.received as f64 - self.last.captured_ms as f64) / 1000.
                ),
                0,
            );
        }
        if !rules.confirmed {
            push("rules","warning","Event assumptions need confirmation","Confirm event rules and measured pit service times in Strategy before relying on projections.".into(),0)
        }
        if age <= 3000 && self.last.authoritative {
            if let Some(range) = strategy.range_laps {
                if range < 2. {
                    push(
                        "range",
                        "critical",
                        "Resource limit approaching",
                        format!(
                            "{:.2} laps estimated to {} depletion, before reserve. Prepare the stop.",
                            range, strategy.limiting
                        ),
                        strategy.clean_laps,
                    )
                }
            }
            if self.last.wheels.iter().any(|w| w.detached) || self.last.detached {
                push(
                    "detached",
                    "critical",
                    "Detached component",
                    "Inspect the car and pit options immediately.".into(),
                    1,
                )
            }
            if self.last.wheels.iter().any(|w| {
                w.temperature_c
                    .is_some_and(|t| t.iter().any(|v| *v > rules.max_tyre_c))
            }) {
                push(
                    "tyre_temp",
                    "warning",
                    "Tyre temperature above limit",
                    format!(
                        "Configured threshold {:.0}°C; review trend and driver feedback.",
                        rules.max_tyre_c
                    ),
                    1,
                )
            }
            if self
                .last
                .wheels
                .iter()
                .any(|w| w.brake_c.is_some_and(|t| t > rules.max_brake_c))
            {
                push(
                    "brake_temp",
                    "warning",
                    "Brake temperature above limit",
                    format!("Configured threshold {:.0}°C.", rules.max_brake_c),
                    1,
                )
            }
            if self.last.penalties > 0 {
                push(
                    "penalty",
                    "warning",
                    "Outstanding penalty",
                    format!(
                        "{} penalty/penalties reported. Confirm type and deadline in LMU.",
                        self.last.penalties
                    ),
                    1,
                )
            }
            if let Some(limit) = rules.max_stint_s {
                let elapsed = (self.last.game_s - self.stint_start).max(0.);
                if elapsed >= limit - 300. {
                    push(
                        "stint",
                        if elapsed >= limit {
                            "critical"
                        } else {
                            "warning"
                        },
                        if elapsed >= limit {
                            "Configured driver-stint limit reached"
                        } else {
                            "Driver stint limit approaching"
                        },
                        format!(
                            "Observed stint {:.0} minutes; configured maximum {:.0} minutes. Review the actual start and planned handover.",
                            elapsed / 60.,
                            limit / 60.
                        ),
                        1,
                    );
                }
            }
            if let Some(limit) = rules.max_drive_s {
                let driven = drive_times.get(&self.last.driver).copied().unwrap_or(0.);
                if driven >= limit - 300. {
                    push(
                        "max_drive",
                        if driven >= limit {
                            "critical"
                        } else {
                            "warning"
                        },
                        if driven >= limit {
                            "Configured total drive-time limit reached"
                        } else {
                            "Driver total-time limit approaching"
                        },
                        format!(
                            "{}: {:.0} corrected observed minutes; configured maximum {:.0} minutes. Arrange a handover and reconcile unrecorded time.",
                            self.last.driver,
                            driven / 60.,
                            limit / 60.
                        ),
                        1,
                    );
                }
            }
            if let Some(next) = rules
                .driver_plan
                .iter()
                .filter(|p| p.entry_id == self.id && p.start_s > self.last.game_s)
                .min_by(|a, b| a.start_s.total_cmp(&b.start_s))
            {
                if next.start_s - self.last.game_s < 300. {
                    push(
                        "handover",
                        "info",
                        "Planned handover approaching",
                        format!(
                            "{} is scheduled at session {:.0} minutes. Confirm readiness and pit plan.",
                            next.driver,
                            next.start_s / 60.
                        ),
                        1,
                    );
                }
            }
            if let Some(minimum) = rules
                .min_drive_s
                .filter(|_| (10..=13).contains(&self.last.session_kind))
            {
                let remaining = self
                    .last
                    .session_remaining_s
                    .unwrap_or((rules.duration_s - self.last.game_s).max(0.));
                for driver in rules
                    .driver_plan
                    .iter()
                    .filter(|p| p.entry_id == self.id)
                    .map(|p| &p.driver)
                    .collect::<std::collections::BTreeSet<_>>()
                {
                    let driven = drive_times.get(driver).copied().unwrap_or(0.);
                    if driven + remaining < minimum {
                        push(
                            &format!("minimum:{driver}"),
                            "critical",
                            "Minimum drive time at risk",
                            format!(
                                "{driver}: {:.0} observed minutes; {:.0} required. Remaining session time cannot cover the shortfall. Reconcile any unrecorded driving time.",
                                driven / 60.,
                                minimum / 60.
                            ),
                            1,
                        );
                    }
                }
            }
            if let Some((used, allocation)) = car_plan.and_then(|p| {
                p.tyres_used
                    .zip(p.tyre_allocation.or(rules.tyre_allocation))
            }) {
                if used.saturating_add(4) > allocation {
                    push(
                        "tyre_inventory",
                        "warning",
                        "No full fresh tyre set in confirmed inventory",
                        format!(
                            "{used} tyres recorded as used from allocation {allocation}. Confirm inventory before calling a tyre change."
                        ),
                        1,
                    );
                }
            }
            if self.last.rain > 0.1 {
                push(
                    "rain",
                    "info",
                    "Rain reported",
                    format!(
                        "Rain {:.0}%; racing-line wetness {:.0}%.",
                        self.last.rain * 100.,
                        self.last.wetness * 100.
                    ),
                    1,
                )
            }
            if let Some(drift) = strategy.pace_drift_s.filter(|v| *v > 0.5) {
                push(
                    "pace_drift",
                    "info",
                    "Observed pace is slowing",
                    format!(
                        "Recent clean laps are {:.2}s slower than the earlier clean-lap window. Review traffic, tyres, fuel and conditions before assigning a cause.",
                        drift
                    ),
                    strategy.clean_laps,
                )
            }
            if let Some(rival) = strategy
                .rivals
                .iter()
                .find(|v| v.gap_s > 0. && v.catch_laps.is_some_and(|n| n < 10.) && !v.in_pits)
            {
                push(
                    "rival_catch",
                    "info",
                    "Class rival is closing",
                    format!(
                        "{} is {:.1}s behind, approximately {:.2}s/lap faster. Constant-pace catch estimate: {:.1} laps.",
                        rival.driver,
                        rival.gap_s,
                        -rival.pace_delta_s,
                        rival.catch_laps.unwrap_or(0.)
                    ),
                    strategy.clean_laps,
                )
            }
        }
        let valid: Vec<_> = self
            .laps
            .iter()
            .filter(|l| l.valid && !l.reference_only)
            .collect();
        let latest = valid.last().copied();
        let reference = valid
            .iter()
            .filter(|lap| {
                lap.samples.len() > 10 && latest.is_some_and(|current| compatible(current, lap))
            })
            .min_by(|a, b| a.time_s.total_cmp(&b.time_s))
            .copied();
        let comparison_sections = reference
            .map(|lap| sections_for(lap, rules))
            .unwrap_or_default();
        let sections = valid
            .last()
            .zip(reference)
            .filter(|(a, b)| a.id != b.id && compatible(a, b))
            .map(|(a, b)| compare_sections(a, b, &comparison_sections))
            .unwrap_or_default();
        // Corner advice requires three eligible repeats against the same compatible reference.
        if let Some(r) = reference.filter(|_| age <= 3000 && self.last.authoritative) {
            let repeats: Vec<_> = valid
                .iter()
                .rev()
                .filter(|lap| {
                    lap.driver == self.last.driver
                        && lap
                            .samples
                            .first()
                            .is_some_and(|s| s.game_s >= self.stint_start)
                })
                .take(3)
                .collect();
            if repeats.len() == 3 && latest.is_some_and(|lap| lap.driver == self.last.driver) {
                for section in sections.iter().filter(|s| s.loss_s > 0.2).take(3) {
                    let repeated = repeats.iter().all(|l| {
                        compatible(l, r)
                            && compare_sections(l, r, &comparison_sections)
                                .iter()
                                .any(|s| s.name == section.name && s.loss_s > 0.2)
                    });
                    if repeated {
                        push(
                            &format!("corner:{}", section.name),
                            "info",
                            &format!("Repeated loss · {}", section.name),
                            format!(
                                "Lap {} · {}: +{:.2}s vs reference. Minimum speed {:+.1} km/h; exit {:+.1} km/h.",
                                latest.unwrap().lap + 1, self.last.driver, section.loss_s,
                                section.min_speed_delta, section.exit_speed_delta
                            ),
                            3,
                        )
                    }
                }
            }
        }
        let laps = self
            .laps
            .iter()
            .rev()
            .take(100)
            .map(Lap::metadata)
            .collect();
        EntryView {
            entry_id: self.id.clone(),
            source_id: self.source.clone(),
            last_received_ms: self.received,
            last_sample: self.last.clone(),
            strategy,
            advice,
            laps,
            drive_times,
            stint_start_s: self.stint_start,
            sections,
            conflict: self.conflict,
        }
    }
}
fn compact(s: &Sample) -> Sample {
    s.trace_sample()
}

pub fn calculate_strategy(s: &Sample, laps: &[&Lap], r: &Rules) -> Strategy {
    let clean: Vec<_> = laps
        .iter()
        .rev()
        .filter(|l| {
            !l.reference_only
                && l.valid
                && l.vehicle == s.vehicle
                && l.track == s.track
                && (l.rain - s.rain).abs() < 0.15
        })
        .take(15)
        .copied()
        .collect();
    let pace = median(clean.iter().map(|l| l.time_s).collect());
    let fuel_values: Vec<_> = clean.iter().filter_map(|l| l.fuel_used_l).collect();
    let ve_values: Vec<_> = clean.iter().filter_map(|l| l.ve_used).collect();
    let fuel = if fuel_values.len() >= 3 {
        median(fuel_values)
    } else {
        None
    };
    let ve = if ve_values.len() >= 3 {
        median(ve_values)
    } else {
        None
    };
    let fr = s
        .fuel_l
        .zip(fuel)
        .filter(|(_, c)| *c > 0.)
        .map(|(v, c)| v / c);
    let vr = s.ve.zip(ve).filter(|(_, c)| *c > 0.).map(|(v, c)| v / c);
    let (range, limiting) = match (fr, vr) {
        (Some(a), Some(b)) => {
            if a < b {
                (Some(a), "fuel")
            } else {
                (Some(b), "virtual energy")
            }
        }
        (Some(a), _) => (Some(a), "fuel"),
        (_, Some(b)) => (Some(b), "virtual energy"),
        _ => (None, "unknown"),
    };
    let mut st = Strategy {
        clean_laps: clean.len(),
        pace_s: pace,
        fuel_per_lap: fuel,
        ve_per_lap: ve,
        range_laps: range,
        limiting: limiting.into(),
        ..Default::default()
    };
    let fuel_safe = s
        .fuel_l
        .zip(fuel)
        .filter(|(_, c)| *c > 0.)
        .map(|(v, c)| (v - r.reserve_fuel_l).max(0.) / c);
    let ve_safe =
        s.ve.zip(ve)
            .filter(|(_, c)| *c > 0.)
            .map(|(v, c)| (v - r.reserve_ve).max(0.) / c);
    let safe_range = match (fuel_safe, ve_safe) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    st.pit_by_lap = safe_range.map(|n| s.lap + (n - r.reserve_laps).floor().max(0.) as i32);
    if clean.len() >= 6 {
        let recent = median(clean.iter().take(3).map(|l| l.time_s).collect());
        let previous = median(clean.iter().skip(3).take(3).map(|l| l.time_s).collect());
        st.pace_drift_s = recent.zip(previous).map(|(a, b)| a - b);
    }
    st.assumptions.push(
        if r.confirmed {
            "Engineer-confirmed event and service assumptions"
        } else {
            "Unconfirmed editable event and pit-time assumptions"
        }
        .into(),
    );
    if !s.authoritative {
        st.assumptions.push("No driver telemetry".into());
        return st;
    }
    if clean.len() < 3 {
        st.assumptions
            .push("Consumption model needs three clean laps".into());
        return st;
    }
    let known_fuel = s.fuel_l.zip(fuel).is_some();
    let known_ve = s.ve.zip(ve).is_some();
    if !known_fuel && !known_ve {
        st.assumptions.push("No usable current resource model: each resource needs its current reading and at least three clean consumption observations".into());
        return st;
    }
    if !known_fuel {
        st.assumptions.push("Fuel is not covered by this forecast: its current reading or three clean consumption observations are unavailable".into());
    }
    if !known_ve {
        st.assumptions.push("Virtual energy is not covered by this forecast: its current reading or three clean consumption observations are unavailable".into());
    }
    let Some(pace) = pace else { return st };
    if r.pit_entry_m.is_some_and(|d| d >= s.track_length_m) {
        st.assumptions.push(
            "Configured pit-entry distance is outside this track; correct it before forecasting"
                .into(),
        );
        return st;
    }
    st.assumptions.push(if r.pit_entry_m.is_some(){"Pit opportunities use the configured pit-entry distance; pit now means the next reachable entry, extend means one further lap"}else{"Pit entry is approximated at start/finish. Calibrate its distance in event settings; pit now means the next entry, extend skips it for one further lap"}.into());
    if let Some(own) = s
        .standings
        .iter()
        .filter(|_| (10..=13).contains(&s.session_kind))
        .find(|v| Some(v.slot) == s.slot)
    {
        let gap = |v: &Standing| {
            v.gap_s
                .or_else(|| (v.position == 1).then_some(0.))
                .map(|g| g + v.laps_behind as f64 * pace)
        };
        let class: Vec<_> = s
            .standings
            .iter()
            .filter(|v| v.class == own.class)
            .collect();
        if let Some(own_gap) = gap(own) {
            if class.iter().all(|v| gap(v).is_some()) {
                st.rejoin_class_position = Some(
                    1 + class
                        .iter()
                        .filter(|v| {
                            v.slot != own.slot && gap(v).unwrap() < own_gap + r.pit_transit_s
                        })
                        .count() as u32,
                );
            } else {
                st.assumptions.push("Class gap data is incomplete: rejoin position is unavailable and unknown-gap rivals are omitted".into());
            }
            st.rivals = class
                .iter()
                .filter(|v| v.slot != own.slot)
                .filter_map(|v| {
                    let other = v.last_lap_s.filter(|t| *t > 20.)?;
                    let relative = gap(v)? - own_gap;
                    let delta = other - pace;
                    let catch = if relative > 0. && delta < -0.05 {
                        Some(relative / -delta)
                    } else if relative < 0. && delta > 0.05 {
                        Some(-relative / delta)
                    } else {
                        None
                    };
                    Some(RivalProjection {
                        driver: v.driver.clone(),
                        position: v.position,
                        gap_s: relative,
                        pace_delta_s: delta,
                        catch_laps: catch,
                        in_pits: v.in_pits,
                    })
                })
                .collect();
        } else {
            st.assumptions.push("Own gap to leader is unavailable: rival catch and rejoin projections are suppressed".into());
        }
    }
    let racing = (10..=13).contains(&s.session_kind);
    let remaining = if racing {
        s.session_remaining_s
            .unwrap_or((r.duration_s - s.game_s).max(0.))
    } else {
        r.duration_s
    };
    if !racing {
        st.assumptions.push("Practice / qualifying rehearsal: uses the configured race duration from now; no rival catch or rejoin projection".into());
    }
    let leader = s
        .standings
        .iter()
        .filter(|_| racing)
        .find(|v| v.position == 1);
    let leader_pace = leader
        .and_then(|v| v.last_lap_s)
        .filter(|v| *v > 20.)
        .unwrap_or(pace);
    let leader_fraction = leader
        .map(|v| (v.distance_m / s.track_length_m.max(1.)).clamp(0., 1.))
        .unwrap_or(0.);
    let leader_to_line = (1. - leader_fraction) * leader_pace;
    let finish_time = if remaining <= leader_to_line {
        leader_to_line
    } else {
        leader_to_line + ((remaining - leader_to_line) / leader_pace).ceil() * leader_pace
    } + if r.extra_lap { leader_pace } else { 0. };
    let part = (s.lap_distance_m / s.track_length_m.max(1.)).clamp(0., 1.);
    let finish_laps = ((finish_time / pace + part).ceil() - part).max(0.);
    st.finish_laps = Some(finish_laps.ceil() as u32);
    st.assumptions.push("Scenarios compare the same nominal forecast distance. Service time may change the actual timed-race lap count; displayed completion times are comparisons, not guaranteed race finish times".into());
    if let Some(v) = s.fuel_l {
        st.fuel_target_per_lap =
            Some((v - r.reserve_fuel_l).max(0.) / (finish_laps + r.reserve_laps).max(1.));
    }
    if let Some(v) = s.ve {
        st.ve_target_per_lap =
            Some((v - r.reserve_ve).max(0.) / (finish_laps + r.reserve_laps).max(1.));
    }
    for (name, offset, save, tyres) in [
        ("Pit now", 0., 1., false),
        ("Extend one lap", 1., 1., false),
        ("Save 3%", 0., 0.97, false),
        ("Change tyres", 0., 1., true),
        ("Driver handover", 0., 1., false),
        ("Repair and service", 0., 1., true),
    ] {
        st.scenarios.push(simulate(
            name,
            s,
            r,
            pace,
            fuel,
            ve,
            finish_time,
            offset,
            save,
            tyres,
        ));
    }
    select_preferred(&mut st);
    st.assumptions.push("Scenarios compare the same forecast race distance. Leader timing, cautions and future pace can change the finish by a lap; reassess continuously".into());
    st.assumptions.push("Opponent pit intentions and tyre/repair pace benefits are not known; service scenarios show costs only".into());
    st.assumptions.push("Saving scenario assumes 3% resource reduction and 1% slower laps; validate against measured saving pace".into());
    st
}
fn select_preferred(st: &mut Strategy) {
    if let Some(p) = st
        .scenarios
        .iter()
        .filter(|s| s.feasible)
        .min_by(|a, b| a.finish_s.total_cmp(&b.finish_s))
    {
        st.preferred_scenario = Some(p.name.clone());
        st.stops = Some(p.stops);
        st.fuel_add_l = Some(p.fuel_add_l);
        st.ve_add = Some(p.ve_add);
    } else {
        st.preferred_scenario = None;
        st.stops = None;
        st.fuel_add_l = None;
        st.ve_add = None;
    }
}
fn distance_to_pit(s: &Sample, r: &Rules) -> f64 {
    if s.in_pits {
        return 0.;
    }
    let length = s.track_length_m.max(1.);
    (r.pit_entry_m.unwrap_or(0.) / length - (s.lap_distance_m / length).clamp(0., 1.))
        .rem_euclid(1.)
}
fn simulate(
    name: &str,
    s: &Sample,
    r: &Rules,
    pace: f64,
    fuel: Option<f64>,
    ve: Option<f64>,
    finish_time: f64,
    offset: f64,
    save: f64,
    tyres: bool,
) -> Scenario {
    let fc = fuel.unwrap_or(0.) * save;
    let ec = ve.unwrap_or(0.) * save;
    // A reported zero for an unmodelled resource is not a depletion estimate.
    // For example, LMU reports zero VE for the recorded LMP2 car. Only resources
    // with learned consumption may constrain this explicitly partial forecast.
    let mut f = if fc > 0. {
        s.fuel_l.unwrap_or(f64::INFINITY)
    } else {
        f64::INFINITY
    };
    let mut e = if ec > 0. {
        s.ve.unwrap_or(f64::INFINITY)
    } else {
        f64::INFINITY
    };
    let mut elapsed = 0.;
    let mut stops = 0;
    let mut first_f = 0.;
    let mut first_e = 0.;
    let mut feasible = true;
    let lap_time = pace * if save < 1. { 1.01 } else { 1. };
    let length = s.track_length_m.max(1.);
    let fraction = (s.lap_distance_m / length).clamp(0., 1.);
    let target_distance = ((finish_time / pace + fraction).ceil() - fraction).max(0.);
    let approach = distance_to_pit(s, r);
    let mut distance = approach.min(target_distance);
    // Apply the same reserves to the initial approach as to subsequent laps.
    // A car already at service may refill immediately, even below reserve.
    if distance > 0.
        && (f < fc * (distance + r.reserve_laps) + r.reserve_fuel_l
            || e < ec * (distance + r.reserve_laps) + r.reserve_ve)
    {
        feasible = false;
    } else {
        f -= fc * distance;
        e -= ec * distance;
        elapsed += lap_time * distance;
    }
    for visit in 0..3000 {
        if !feasible || distance >= target_distance - 0.000001 {
            break;
        }
        let progress = (target_distance - distance).min(1.);
        let need_f = fc * (progress + r.reserve_laps) + r.reserve_fuel_l;
        let need_e = ec * (progress + r.reserve_laps) + r.reserve_ve;
        let low = f < need_f || e < need_e;
        // Resources can be added only at a pit opportunity. Extending skips the
        // first opportunity, so a low-resource extension cannot be rescued early.
        if low && (visit as f64) < offset {
            feasible = false;
            break;
        }
        if low || visit as f64 == offset {
            let laps_needed = target_distance - distance + r.reserve_laps;
            let target_f =
                (fc * laps_needed + r.reserve_fuel_l).min(s.fuel_capacity_l.unwrap_or(f));
            let target_e = (ec * laps_needed + r.reserve_ve).min(1.);
            let add_f = if fc > 0. { (target_f - f).max(0.) } else { 0. };
            let add_e = if ec > 0. { (target_e - e).max(0.) } else { 0. };
            let swap = name == "Driver handover" && stops == 0;
            let repair = if name == "Repair and service" && stops == 0 {
                r.repair_s
            } else {
                0.
            };
            if add_f == 0. && add_e == 0. && !tyres && !swap && repair == 0. {
                if low {
                    feasible = false;
                    break;
                }
            } else {
                let resource = (add_f / r.fuel_l_per_s).max(add_e / r.ve_per_s);
                let work = (if tyres { r.tyre_change_s } else { 0. }).max(if swap {
                    r.driver_change_s
                } else {
                    0.
                });
                elapsed += r.pit_transit_s
                    + if r.parallel_service {
                        resource.max(work)
                    } else {
                        resource + work
                    }
                    + repair;
                f += add_f;
                e += add_e;
                stops += 1;
                if stops == 1 {
                    first_f = add_f;
                    first_e = add_e;
                }
            }
        }
        if f < need_f || e < need_e {
            feasible = false;
            break;
        }
        f -= fc * progress;
        e -= ec * progress;
        elapsed += lap_time * progress;
        distance += progress;
        if visit == 2999 {
            feasible = false;
        }
    }
    Scenario {
        name: name.into(),
        stops,
        finish_s: elapsed,
        fuel_add_l: first_f,
        ve_add: first_e,
        feasible,
        reason: if feasible {
            "Forecast under displayed assumptions"
        } else {
            "Cannot safely reach a required pit opportunity or complete this forecast"
        }
        .into(),
    }
}
pub fn compatible(a: &Lap, b: &Lap) -> bool {
    a.track == b.track
        && a.vehicle == b.vehicle
        && a.compound == b.compound
        && (a.rain - b.rain).abs() < 0.1
        && a.fuel_start_l
            .zip(b.fuel_start_l)
            .is_some_and(|(a, b)| (a - b).abs() < 15.)
}
fn at(samples: &[Sample], d: f64) -> Option<f64> {
    let i = samples.partition_point(|s| s.lap_distance_m < d);
    if i == 0 || i >= samples.len() {
        return None;
    }
    let a = &samples[i - 1];
    let b = &samples[i];
    let span = b.lap_distance_m - a.lap_distance_m;
    if span <= 0. || span > 100. {
        return None;
    }
    Some(a.game_s + (b.game_s - a.game_s) * (d - a.lap_distance_m) / span)
}
pub fn sections_match(track: &str, configured: &str) -> bool {
    !configured.trim().is_empty()
        && track
            .to_lowercase()
            .contains(&configured.trim().to_lowercase())
}
pub fn sections_for(reference: &Lap, rules: &Rules) -> Vec<Section> {
    if sections_match(&reference.track, &rules.sections_track) && !rules.sections.is_empty() {
        return rules.sections.clone();
    }
    let points = &reference.samples;
    let Some(first) = points.first() else {
        return vec![];
    };
    let Some(last) = points.last() else {
        return vec![];
    };
    let mut runs: Vec<(f64, f64, f64, usize)> = Vec::new();
    for sample in points
        .iter()
        .filter(|s| s.brake > 0.1 && s.speed_kmh > 30. && s.lap_distance_m >= 0.)
    {
        if let Some(run) = runs
            .last_mut()
            .filter(|run| sample.game_s - run.2 <= 0.8 || sample.lap_distance_m - run.1 < 120.)
        {
            run.1 = sample.lap_distance_m;
            run.2 = sample.game_s;
            run.3 += 1;
        } else {
            runs.push((
                sample.lap_distance_m,
                sample.lap_distance_m,
                sample.game_s,
                1,
            ));
        }
    }
    let runs: Vec<_> = runs
        .into_iter()
        .filter(|run| run.3 >= 3 && run.1 - run.0 >= 10.)
        .take(32)
        .collect();
    runs.iter()
        .enumerate()
        .filter_map(|(index, run)| {
            let mut start = (run.0 - 100.).max(first.lap_distance_m + 1.);
            let mut end = (run.1 + 200.).min(last.lap_distance_m - 1.);
            if index > 0 {
                start = start.max((runs[index - 1].1 + run.0) / 2.);
            }
            if let Some(next) = runs.get(index + 1) {
                end = end.min((run.1 + next.0) / 2.);
            }
            (end > start + 30.).then(|| Section {
                name: format!("Brake zone {} · {:.0}–{:.0} m", index + 1, start, end),
                start_m: start,
                end_m: end,
            })
        })
        .collect()
}
pub fn compare_sections(a: &Lap, b: &Lap, sections: &[Section]) -> Vec<SectionComparison> {
    sections
        .iter()
        .filter_map(|sec| {
            let ca: Vec<_> = a
                .samples
                .iter()
                .filter(|s| s.lap_distance_m >= sec.start_m && s.lap_distance_m <= sec.end_m)
                .collect();
            let cb: Vec<_> = b
                .samples
                .iter()
                .filter(|s| s.lap_distance_m >= sec.start_m && s.lap_distance_m <= sec.end_m)
                .collect();
            let ta = at(&a.samples, sec.end_m)? - at(&a.samples, sec.start_m)?;
            let tb = at(&b.samples, sec.end_m)? - at(&b.samples, sec.start_m)?;
            let brake =
                |v: &Vec<&Sample>| v.iter().find(|s| s.brake > 0.1).map(|s| s.lap_distance_m);
            let throttle = |v: &Vec<&Sample>| {
                let min = v
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| a.speed_kmh.total_cmp(&b.speed_kmh))
                    .map(|(i, _)| i)?;
                v.iter()
                    .skip(min)
                    .find(|s| s.throttle > 0.95)
                    .map(|s| s.lap_distance_m)
            };
            let min =
                |v: &Vec<&Sample>| v.iter().map(|s| s.speed_kmh).fold(f64::INFINITY, f64::min);
            Some(SectionComparison {
                name: sec.name.clone(),
                loss_s: ta - tb,
                brake_delta_m: brake(&ca).zip(brake(&cb)).map(|(a, b)| a - b),
                throttle_delta_m: throttle(&ca).zip(throttle(&cb)).map(|(a, b)| a - b),
                min_speed_delta: min(&ca) - min(&cb),
                exit_speed_delta: ca.last()?.speed_kmh - cb.last()?.speed_kmh,
                current_s: ta,
                reference_s: tb,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finish_line_capture_gap_excludes_both_affected_laps() {
        for gap in [0.2, 1.5] {
            let mut entry = Entry::new("c".into(), "pc".into(), sample(0., 0, 0., 80.), 0);
            for i in 1..300 {
                let t = i as f64 / 10.;
                entry.update(
                    "pc",
                    sample(t, 0, t * 100., 80. - t * 0.1),
                    (t * 1000.) as u64,
                );
            }
            let mut crossing = sample(29.9 + gap, 1, 5., 77.);
            crossing.last_lap_s = Some(30.);
            entry.update("pc", crossing, 32000);
            assert_eq!(entry.laps.len(), 1);
            assert_eq!(entry.laps[0].valid, gap <= 1.);
            assert_eq!(entry.current_bad, gap > 1.);
            if gap > 1. {
                assert_eq!(entry.laps[0].reason, "Pit, yellow, invalid lap or data gap");
                let view = entry.view(&Rules::default(), 32000);
                assert_eq!(view.strategy.clean_laps, 0);
            }
        }
    }
    fn sample(t: f64, lap: i32, d: f64, f: f64) -> Sample {
        Sample {
            game_s: t,
            captured_ms: (t * 1000.) as u64,
            lap,
            lap_distance_m: d,
            fuel_l: Some(f),
            fuel_capacity_l: Some(100.),
            track: "Monza".into(),
            vehicle: "GT3".into(),
            driver: "A".into(),
            track_length_m: 5793.,
            authoritative: true,
            valid_lap: true,
            ..Default::default()
        }
    }
    #[test]
    fn late_former_driver_packet_cannot_reverse_a_handover() {
        let mut e = Entry::new("c".into(), "pc-a".into(), sample(100., 0, 100., 80.), 1000);
        let mut next = sample(101., 0, 200., 79.);
        next.driver = "B".into();
        e.update("pc-b", next, 2000);
        let late = sample(100.5, 0, 150., 79.5);
        e.update("pc-a", late, 2100);
        assert_eq!(e.last.driver, "B");
        assert_eq!(e.last.game_s, 101.);
        assert_eq!(e.stint_start, 101.);
        assert!(e.conflict);
    }
    #[test]
    fn initial_garage_time_is_not_a_driver_stint() {
        let mut garage = sample(0., 0, 0., 80.);
        garage.authoritative = false;
        let mut e = Entry::new("c".into(), "pc".into(), garage, 1000);
        e.update("pc", sample(15., 0, 0., 80.), 16000);
        assert_eq!(e.stint_start, 15.);
        assert!(e.drive_times.is_empty());
    }
    #[test]
    fn same_driver_pit_stop_does_not_reset_driver_stint() {
        let mut e = Entry::new("c".into(), "a".into(), sample(100., 0, 0., 80.), 1000);
        let mut inside = sample(101., 0, 100., 79.);
        inside.in_pits = true;
        e.update("a", inside, 1100);
        e.update("a", sample(102., 0, 200., 79.), 1200);
        assert_eq!(e.stint_start, 100.);
        let mut replacement = sample(103., 0, 210., 79.);
        replacement.driver = "B".into();
        e.update("b", replacement, 1300);
        assert_eq!(e.stint_start, 103.);
    }
    #[test]
    fn unknown_rival_gaps_are_not_treated_as_zero() {
        let mut s = sample(100., 4, 100., 80.);
        s.session_kind = 10;
        s.slot = Some(1);
        s.standings = vec![
            Standing {
                slot: 1,
                driver: "A".into(),
                class: "GT3".into(),
                position: 2,
                gap_s: Some(10.),
                last_lap_s: Some(110.),
                ..Default::default()
            },
            Standing {
                slot: 2,
                driver: "Unknown".into(),
                class: "GT3".into(),
                position: 3,
                gap_s: None,
                last_lap_s: Some(109.),
                ..Default::default()
            },
            Standing {
                slot: 3,
                driver: "Leader".into(),
                class: "GT3".into(),
                position: 1,
                gap_s: None,
                last_lap_s: Some(111.),
                ..Default::default()
            },
        ];
        let lap = Lap {
            id: "x".into(),
            entry_id: "c".into(),
            driver: "A".into(),
            vehicle: "GT3".into(),
            track: "Monza".into(),
            lap: 1,
            time_s: 110.,
            fuel_used_l: Some(3.),
            ve_used: None,
            valid: true,
            reference_only: false,
            reason: "Clean".into(),
            fuel_start_l: Some(80.),
            rain: 0.,
            compound: 0,
            samples: vec![],
        };
        let view = calculate_strategy(&s, &[&lap, &lap, &lap], &Rules::default());
        assert!(view.rejoin_class_position.is_none());
        assert_eq!(view.rivals.len(), 1);
        assert_eq!(view.rivals[0].driver, "Leader");
        assert_eq!(view.rivals[0].gap_s, -10.);
        s.standings[0].gap_s = None;
        let view = calculate_strategy(&s, &[&lap, &lap, &lap], &Rules::default());
        assert!(view.rivals.is_empty() && view.rejoin_class_position.is_none());
    }
    #[test]
    fn stripped_trace_has_no_scoring_allocation_and_rejected_feed_preserves_scores() {
        let mut s = sample(0., 0, 0., 80.);
        s.standings = vec![Standing::default(); 38];
        s.supplemental = Some(serde_json::json!({"observed_ms":0,"forecast":[1,2,3]}));
        let stripped = compact(&s);
        assert_eq!(stripped.standings.capacity(), 0);
        assert!(stripped.supplemental.is_none());
        assert_eq!(s.standings.len(), 38);
        let mut e = Entry::new("c".into(), "pc".into(), s, 1000);
        let mut rejected = sample(0.1, 0, 10., 80.);
        rejected.authoritative = false;
        e.update("spectator", rejected, 1100);
        assert_eq!(e.last.standings.len(), 38);
        e.update("pc", sample(0.2, 0, 20., 80.), 1200);
        assert_eq!(e.last.standings.len(), 38);
        assert!(e.current.iter().all(|s| s.standings.capacity() == 0));
    }
    fn clean_model_lap() -> Lap {
        Lap {
            id: "model".into(),
            entry_id: "c".into(),
            driver: "A".into(),
            vehicle: "GT3".into(),
            track: "Monza".into(),
            lap: 1,
            time_s: 110.,
            fuel_used_l: Some(3.),
            ve_used: None,
            valid: true,
            reference_only: false,
            reason: "Clean".into(),
            fuel_start_l: Some(80.),
            rain: 0.,
            compound: 0,
            samples: vec![],
        }
    }
    #[test]
    fn automatic_braking_zones_use_the_reference_and_preserve_manual_sections() {
        let mut reference = clean_model_lap();
        reference.track = "Bahrain International Circuit".into();
        reference.samples = (0..600)
            .map(|i| {
                let mut point = sample(i as f64 / 10., 1, i as f64 * 10., 80.);
                point.track = reference.track.clone();
                point.speed_kmh = if (50..70).contains(&i) || (300..320).contains(&i) {
                    80.
                } else {
                    200.
                };
                point.brake = if (40..65).contains(&i) || (290..315).contains(&i) {
                    0.8
                } else {
                    0.
                };
                point
            })
            .collect();
        let mut rules = Rules::default();
        let zones = sections_for(&reference, &rules);
        assert_eq!(zones.len(), 2);
        assert!(zones[0].start_m < 400. && zones[0].end_m > 640.);
        assert!(zones[0].end_m <= zones[1].start_m);
        let same = compare_sections(&reference, &reference, &zones);
        assert_eq!(same.len(), 2);
        assert!(same
            .iter()
            .all(|s| s.loss_s.abs() < 1e-9 && s.brake_delta_m == Some(0.)));
        rules.sections_track = "Bahrain".into();
        rules.sections = vec![Section {
            name: "T1".into(),
            start_m: 200.,
            end_m: 900.,
        }];
        assert_eq!(sections_for(&reference, &rules)[0].name, "T1");
    }
    #[test]
    fn spectator_retains_learned_consumption_without_requesting_more_laps() {
        let mut s = sample(100., 4, 100., 80.);
        s.authoritative = false;
        s.fuel_l = None;
        let lap = clean_model_lap();
        let st = calculate_strategy(&s, &[&lap, &lap, &lap], &Rules::default());
        assert_eq!(st.clean_laps, 3);
        assert_eq!(st.fuel_per_lap, Some(3.));
        assert!(st
            .assumptions
            .iter()
            .any(|text| text == "No driver telemetry"));
        assert!(!st
            .assumptions
            .iter()
            .any(|text| text.contains("needs three")));
        assert!(st.scenarios.is_empty() && st.range_laps.is_none());
    }
    #[test]
    fn missing_consumption_cannot_produce_a_zero_stop_forecast() {
        let s = sample(100., 4, 100., 80.);
        let mut lap = clean_model_lap();
        lap.fuel_used_l = None;
        let st = calculate_strategy(&s, &[&lap, &lap, &lap], &Rules::default());
        assert!(st.scenarios.is_empty() && st.stops.is_none());
        lap.fuel_used_l = Some(3.);
        let mut missing = lap.clone();
        missing.fuel_used_l = None;
        let st = calculate_strategy(&s, &[&lap, &lap, &missing], &Rules::default());
        assert!(st.scenarios.is_empty());
    }
    #[test]
    fn preferred_fill_is_reselected_after_constraints_and_cleared_when_infeasible() {
        let mut st = Strategy::default();
        st.scenarios = vec![
            Scenario {
                name: "Unsafe".into(),
                stops: 1,
                finish_s: 10.,
                fuel_add_l: 2.,
                ve_add: 0.,
                feasible: false,
                reason: "constraint".into(),
            },
            Scenario {
                name: "Safe".into(),
                stops: 2,
                finish_s: 20.,
                fuel_add_l: 10.,
                ve_add: 0.,
                feasible: true,
                reason: "".into(),
            },
        ];
        select_preferred(&mut st);
        assert_eq!(st.preferred_scenario.as_deref(), Some("Safe"));
        assert_eq!(st.fuel_add_l, Some(10.));
        st.scenarios[1].feasible = false;
        select_preferred(&mut st);
        assert!(st.preferred_scenario.is_none() && st.fuel_add_l.is_none() && st.stops.is_none());
    }
    #[test]
    fn extension_limit_includes_travel_to_entry_and_handover_must_reach_it() {
        let mut e = Entry::new("c".into(), "pc".into(), sample(100., 4, 2896.5, 80.), 1000);
        let lap = clean_model_lap();
        e.laps = std::collections::VecDeque::from([lap.clone(), lap.clone(), lap]);
        e.drive_times.insert("A".into(), 850.);
        let mut r = Rules::default();
        r.max_drive_s = Some(1000.);
        let st = e.view(&r, 1000).strategy;
        assert!(
            !st.scenarios
                .iter()
                .find(|s| s.name == "Extend one lap")
                .unwrap()
                .feasible
        );
        assert!(
            st.scenarios
                .iter()
                .find(|s| s.name == "Driver handover")
                .unwrap()
                .feasible
        );
        e.drive_times.insert("A".into(), 980.);
        assert!(
            !e.view(&r, 1000)
                .strategy
                .scenarios
                .iter()
                .find(|s| s.name == "Driver handover")
                .unwrap()
                .feasible
        );
        e.drive_times.insert("A".into(), 1001.);
        assert!(e
            .view(&r, 1000)
            .advice
            .iter()
            .any(|a| a.id.ends_with(":max_drive") && a.severity == "critical"));
    }
    #[test]
    fn missing_remaining_clock_is_not_assumed_zero_for_minimum_drive_time() {
        let mut s = sample(100., 4, 100., 80.);
        s.session_kind = 10;
        let e = Entry::new("c".into(), "pc".into(), s, 1000);
        let mut r = Rules::default();
        r.min_drive_s = Some(600.);
        r.driver_plan.push(DriverPlan {
            entry_id: "c".into(),
            driver: "B".into(),
            start_s: 1000.,
            end_s: 2000.,
        });
        assert!(!e
            .view(&r, 1000)
            .advice
            .iter()
            .any(|a| a.id.contains(":minimum:")));
        r.duration_s = 150.;
        assert!(e
            .view(&r, 1000)
            .advice
            .iter()
            .any(|a| a.id.contains(":minimum:")));
    }
    #[test]
    fn corner_alerts_require_fresh_current_driver_repeats_in_this_stint() {
        let lap = |number: i32, start: f64, time: f64, driver: &str| {
            let mut lap = clean_model_lap();
            lap.id = format!("lap-{number}");
            lap.lap = number;
            lap.driver = driver.into();
            lap.time_s = time;
            lap.samples = (0..=60)
                .map(|i| {
                    let distance = i as f64 / 60. * 5793.;
                    sample(start + distance / 5793. * time, number, distance, 80.)
                })
                .collect();
            lap
        };
        let mut e = Entry::new("c".into(), "pc-a".into(), sample(500., 4, 100., 80.), 1000);
        e.stint_start = 0.;
        e.laps = VecDeque::from([
            lap(0, 10., 110., "Reference"),
            lap(1, 120., 112., "A"),
            lap(2, 232., 112., "A"),
            lap(3, 344., 112., "A"),
        ]);
        let mut rules = Rules::default();
        rules.sections_track = "Monza".into();
        rules.sections = vec![Section {
            name: "Measured section".into(),
            start_m: 100.,
            end_m: 900.,
        }];
        let has_corner = |e: &Entry, now| {
            e.view(&rules, now)
                .advice
                .iter()
                .any(|a| a.id.contains(":corner:"))
        };
        assert!(has_corner(&e, 1000));
        assert!(!has_corner(&e, 5000));
        e.last.authoritative = false;
        assert!(!has_corner(&e, 1000));
        let mut next = sample(501., 4, 110., 80.);
        next.driver = "B".into();
        e.update("pc-b", next, 1100);
        assert!(
            !has_corner(&e, 1100),
            "New driver received the former driver's coaching"
        );
        e.update("pc-a", sample(502., 4, 120., 80.), 1200);
        assert!(
            !has_corner(&e, 1200),
            "Returning driver reused repeats from an earlier stint"
        );
    }
    #[test]
    fn corner_sections_require_the_configured_track() {
        let mut current = clean_model_lap();
        current.id = "current".into();
        current.track = "Sebring".into();
        current.time_s = 112.;
        current.samples = (0..60)
            .map(|i| sample(i as f64 * 100. / 5793. * 112., 1, i as f64 * 100., 80.))
            .collect();
        let mut reference = current.clone();
        reference.id = "reference".into();
        reference.time_s = 110.;
        for s in &mut reference.samples {
            s.game_s = s.lap_distance_m / 5793. * 110.;
        }
        let mut s = sample(500., 5, 200., 80.);
        s.track = "Sebring".into();
        let mut e = Entry::new("c".into(), "pc".into(), s, 1000);
        e.laps = VecDeque::from([reference, current.clone(), current.clone(), current]);
        let mut r = Rules::default();
        r.sections = vec![Section {
            name: "Test section".into(),
            start_m: 100.,
            end_m: 900.,
        }];
        assert!(e.view(&r, 1000).sections.is_empty());
        r.sections_track = "Sebring".into();
        assert!(!e.view(&r, 1000).sections.is_empty());
    }
    #[test]
    fn median_rejects_nan() {
        assert_eq!(median(vec![1., 3., f64::NAN, 2.]), Some(2.));
    }
    #[test]
    fn gradual_refill_excluded() {
        let mut e = Entry::new("car".into(), "pc".into(), sample(0., 0, 0., 30.), 0);
        for i in 1..20 {
            let mut s = sample(i as f64 / 10., 0, i as f64 * 10., 30. + i as f64 / 10.);
            s.in_pits = true;
            e.update("pc", s, i * 100);
        }
        assert!(e.current_bad);
    }
    #[test]
    fn spectator_does_not_replace_driver() {
        let mut e = Entry::new("c".into(), "a".into(), sample(1., 0, 0., 80.), 1000);
        let mut s = sample(2., 0, 100., 50.);
        s.authoritative = false;
        e.update("b", s, 1100);
        assert_eq!(e.source, "a");
        assert_eq!(e.last.fuel_l, Some(80.));
    }
    #[test]
    fn out_of_order_is_reset_not_negative_drive_time() {
        let mut e = Entry::new("c".into(), "a".into(), sample(20., 0, 10., 80.), 20000);
        e.update("a", sample(2., 0, 0., 100.), 21000);
        assert!(e.drive_times.is_empty());
    }
    #[test]
    fn resource_cannot_be_added_before_reaching_pit_entry() {
        let mut s = sample(100., 4, 579.3, 0.5);
        let rules = Rules::default();
        assert!(
            !simulate(
                "Pit now",
                &s,
                &rules,
                110.,
                Some(3.),
                None,
                1000.,
                0.,
                1.,
                false
            )
            .feasible
        );
        s.in_pits = true;
        assert!(
            simulate(
                "Pit now",
                &s,
                &rules,
                110.,
                Some(3.),
                None,
                1000.,
                0.,
                1.,
                false
            )
            .feasible
        );
        s.in_pits = false;
        // Enough for the nearby entry and default 1 lap + 1 L reserve,
        // but not enough to skip that entry and travel one extra lap.
        s.fuel_l = Some(4.5);
        s.lap_distance_m = 0.;
        let mut measured = rules.clone();
        measured.pit_entry_m = Some(500.);
        assert!(
            simulate(
                "Pit now",
                &s,
                &measured,
                110.,
                Some(3.),
                None,
                1000.,
                0.,
                1.,
                false
            )
            .feasible
        );
        assert!(
            !simulate(
                "Extend one lap",
                &s,
                &measured,
                110.,
                Some(3.),
                None,
                1000.,
                1.,
                1.,
                false
            )
            .feasible
        );
    }
    #[test]
    fn unmodelled_zero_resource_does_not_block_the_available_resource_forecast() {
        let mut s = sample(100., 4, 100., 80.);
        s.ve = Some(0.);
        let mut laps = vec![clean_model_lap(); 3];
        let refs: Vec<_> = laps.iter().collect();
        let st = calculate_strategy(&s, &refs, &Rules::default());
        assert_eq!(st.limiting, "fuel");
        assert!(st.scenarios.iter().any(|scenario| scenario.feasible));
        assert!(st
            .assumptions
            .iter()
            .any(|a| a.starts_with("Virtual energy is not covered")));
        for lap in &mut laps {
            lap.fuel_used_l = None;
            lap.ve_used = Some(0.1);
        }
        s.fuel_l = Some(0.);
        s.ve = Some(0.8);
        let refs: Vec<_> = laps.iter().collect();
        let st = calculate_strategy(&s, &refs, &Rules::default());
        assert_eq!(st.limiting, "virtual energy");
        assert!(st.scenarios.iter().any(|scenario| scenario.feasible));
        assert!(st
            .assumptions
            .iter()
            .any(|a| a.starts_with("Fuel is not covered")));
    }
    #[test]
    fn first_pit_approach_preserves_fuel_and_energy_reserves() {
        let mut rules = Rules::default();
        rules.reserve_laps = 0.2;
        rules.reserve_fuel_l = 1.;
        rules.reserve_ve = 0.15;
        let mut s = sample(100., 4, 5793. / 2., 3.);
        let feasible = |s: &Sample, fuel, ve| {
            simulate("Pit now", s, &rules, 110., fuel, ve, 1000., 0., 1., false).feasible
        };
        // Half a lap costs 1.5 L; the 1 L + 0.2 lap reserve needs 1.6 L.
        assert!(!feasible(&s, Some(3.), None));
        s.fuel_l = Some(3.2);
        assert!(feasible(&s, Some(3.), None));
        s.fuel_l = Some(80.);
        s.ve = Some(0.2);
        // Energy must cover 0.05 approach + 0.02 lap reserve + 0.15 absolute.
        assert!(!feasible(&s, Some(3.), Some(0.1)));
        s.ve = Some(0.23);
        assert!(feasible(&s, Some(3.), Some(0.1)));
        s.in_pits = true;
        s.fuel_l = Some(0.1);
        s.ve = Some(0.01);
        assert!(feasible(&s, Some(3.), Some(0.1)));
    }
    #[test]
    fn depleted_extension_is_infeasible() {
        let s = sample(100., 4, 100., 0.5);
        let p = simulate(
            "Extend one lap",
            &s,
            &Rules::default(),
            110.,
            Some(3.),
            None,
            1000.,
            1.,
            1.,
            false,
        );
        assert!(!p.feasible);
    }
    #[test]
    fn service_cost_cannot_improve_fixed_distance_forecast() {
        let s = sample(100., 4, 100., 20.);
        let a = Rules::default();
        let mut b = a.clone();
        b.pit_transit_s += 100.;
        let fast = simulate(
            "Pit now",
            &s,
            &a,
            110.,
            Some(3.),
            None,
            1000.,
            0.,
            1.,
            false,
        );
        let slow = simulate(
            "Pit now",
            &s,
            &b,
            110.,
            Some(3.),
            None,
            1000.,
            0.,
            1.,
            false,
        );
        assert!(fast.feasible && slow.feasible);
        assert!(slow.finish_s >= fast.finish_s + 99.);
    }
    #[test]
    fn missing_fuel_conditions_do_not_authorize_corner_advice() {
        let lap = Lap {
            id: "x".into(),
            entry_id: "c".into(),
            driver: "a".into(),
            vehicle: "v".into(),
            track: "t".into(),
            lap: 1,
            time_s: 110.,
            fuel_used_l: None,
            ve_used: None,
            valid: true,
            reference_only: false,
            reason: "Clean".into(),
            fuel_start_l: None,
            rain: 0.,
            compound: 0,
            samples: vec![],
        };
        assert!(!compatible(&lap, &lap));
    }
    #[test]
    fn tyre_only_and_driver_stops_cost_time_without_refuelling() {
        let s = sample(100., 4, 100., 100.);
        let mut rules = Rules::default();
        rules.driver_change_s = 25.;
        let baseline = simulate(
            "Pit now",
            &s,
            &rules,
            110.,
            Some(3.),
            None,
            200.,
            0.,
            1.,
            false,
        );
        let tyres = simulate(
            "Change tyres",
            &s,
            &rules,
            110.,
            Some(3.),
            None,
            200.,
            0.,
            1.,
            true,
        );
        let driver = simulate(
            "Driver handover",
            &s,
            &rules,
            110.,
            Some(3.),
            None,
            200.,
            0.,
            1.,
            false,
        );
        assert_eq!(baseline.stops, 0);
        assert_eq!(tyres.stops, 1);
        assert_eq!(driver.stops, 1);
        assert!(
            (tyres.finish_s - baseline.finish_s - rules.pit_transit_s - rules.tyre_change_s).abs()
                < 0.01
        );
        assert!(
            (driver.finish_s - baseline.finish_s - rules.pit_transit_s - rules.driver_change_s)
                .abs()
                < 0.01
        );
    }
    #[test]
    fn small_splash_does_not_require_five_litre_jump() {
        let mut e = Entry::new("c".into(), "a".into(), sample(0., 0, 0., 1.), 0);
        let mut s = sample(0.1, 0, 0., 1.1);
        s.in_pits = true;
        e.update("a", s, 100);
        assert!(e.current_bad);
    }
    #[test]
    fn corrections_affect_limits_without_mutating_recorded_time() {
        let mut e = Entry::new("c".into(), "pc".into(), sample(100., 4, 100., 80.), 1000);
        e.drive_times.insert("A".into(), 100.);
        let mut rules = Rules::default();
        rules.max_drive_s = Some(600.);
        rules
            .drive_time_adjustments
            .insert("c".into(), BTreeMap::from([("A".into(), 250.)]));
        let view = e.view(&rules, 1000);
        assert_eq!(view.drive_times["A"], 350.);
        assert!(view.advice.iter().any(|a| a.id.ends_with(":max_drive")));
        assert_eq!(e.drive_times["A"], 100.);
        rules
            .drive_time_adjustments
            .get_mut("c")
            .unwrap()
            .insert("A".into(), -500.);
        assert_eq!(e.view(&rules, 1000).drive_times["A"], 0.);
    }
}
