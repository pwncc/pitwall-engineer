use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROTOCOL: u32 = 1;
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn new_id() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    pub endpoint: String,
    pub certificate_der: String,
    pub token: String,
    pub source_id: String,
    pub event_id: String,
    pub entry_id: String,
    pub label: String,
    #[serde(default)]
    pub legacy: bool,
    #[serde(default)]
    pub rest_supplement: bool,
    #[serde(default)]
    pub shared_package: bool,
    #[serde(default)]
    pub enrollment_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Wheel {
    pub temperature_c: Option<[f64; 3]>,
    pub carcass_c: Option<f64>,
    pub pressure_kpa: Option<f64>,
    pub remaining: Option<f64>,
    pub brake_c: Option<f64>,
    pub detached: bool,
    pub compound: u8,
    pub suspension_m: f64,
    pub ride_height_m: f64,
    pub rotation: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Standing {
    pub slot: i32,
    pub driver: String,
    pub vehicle: String,
    pub class: String,
    pub position: u8,
    pub laps: i32,
    pub distance_m: f64,
    pub last_lap_s: Option<f64>,
    pub best_lap_s: Option<f64>,
    pub gap_s: Option<f64>,
    pub laps_behind: i32,
    pub in_pits: bool,
    pub penalties: i16,
    pub x: f64,
    pub z: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sample {
    pub captured_ms: u64,
    pub game_s: f64,
    pub track: String,
    pub track_length_m: f64,
    pub session_kind: i32,
    pub session_remaining_s: Option<f64>,
    pub phase: u8,
    pub slot: Option<i32>,
    pub driver: String,
    pub vehicle: String,
    pub class: String,
    pub lap: i32,
    pub lap_distance_m: f64,
    pub lap_start_s: f64,
    pub last_lap_s: Option<f64>,
    pub authoritative: bool,
    pub valid_lap: bool,
    pub in_pits: bool,
    pub pit_state: u8,
    pub speed_kmh: f64,
    pub rpm: f64,
    pub gear: i32,
    pub throttle: f64,
    pub brake: f64,
    pub steering: f64,
    pub clutch: f64,
    pub x: f64,
    pub z: f64,
    pub accel: [f64; 3],
    pub fuel_l: Option<f64>,
    pub fuel_capacity_l: Option<f64>,
    pub ve: Option<f64>,
    pub soc: Option<f64>,
    pub water_c: Option<f64>,
    pub oil_c: Option<f64>,
    pub wheels: [Wheel; 4],
    pub tc: u8,
    pub abs: u8,
    pub tc_active: bool,
    pub abs_active: bool,
    pub brake_bias_front: f64,
    pub motor_map: u8,
    pub migration: u8,
    pub regen_kw: f64,
    pub track_limits: u8,
    pub impact: f64,
    pub impact_s: f64,
    pub dents: [u8; 8],
    pub detached: bool,
    pub rain: f64,
    pub wetness: f64,
    pub air_c: f64,
    pub track_c: f64,
    pub wind: [f64; 3],
    pub yellow: bool,
    pub penalties: i16,
    #[serde(default)]
    pub standings: Vec<Standing>,
    #[serde(default)]
    pub supplemental: Option<serde_json::Value>,
}
impl Sample {
    /// A trace copy excludes scoring and supplementary payloads without cloning them.
    pub fn trace_sample(&self) -> Self {
        Self {
            captured_ms: self.captured_ms,
            game_s: self.game_s,
            track: self.track.clone(),
            track_length_m: self.track_length_m,
            session_kind: self.session_kind,
            session_remaining_s: self.session_remaining_s,
            phase: self.phase,
            slot: self.slot,
            driver: self.driver.clone(),
            vehicle: self.vehicle.clone(),
            class: self.class.clone(),
            lap: self.lap,
            lap_distance_m: self.lap_distance_m,
            lap_start_s: self.lap_start_s,
            last_lap_s: self.last_lap_s,
            authoritative: self.authoritative,
            valid_lap: self.valid_lap,
            in_pits: self.in_pits,
            pit_state: self.pit_state,
            speed_kmh: self.speed_kmh,
            rpm: self.rpm,
            gear: self.gear,
            throttle: self.throttle,
            brake: self.brake,
            steering: self.steering,
            clutch: self.clutch,
            x: self.x,
            z: self.z,
            accel: self.accel,
            fuel_l: self.fuel_l,
            fuel_capacity_l: self.fuel_capacity_l,
            ve: self.ve,
            soc: self.soc,
            water_c: self.water_c,
            oil_c: self.oil_c,
            wheels: self.wheels.clone(),
            tc: self.tc,
            abs: self.abs,
            tc_active: self.tc_active,
            abs_active: self.abs_active,
            brake_bias_front: self.brake_bias_front,
            motor_map: self.motor_map,
            migration: self.migration,
            regen_kw: self.regen_kw,
            track_limits: self.track_limits,
            impact: self.impact,
            impact_s: self.impact_s,
            dents: self.dents,
            detached: self.detached,
            rain: self.rain,
            wetness: self.wetness,
            air_c: self.air_c,
            track_c: self.track_c,
            wind: self.wind,
            yellow: self.yellow,
            penalties: self.penalties,
            standings: Vec::new(),
            supplemental: None,
        }
    }
    pub fn validate(&self) -> bool {
        [
            self.game_s,
            self.track_length_m,
            self.speed_kmh,
            self.throttle,
            self.brake,
            self.steering,
            self.x,
            self.z,
            self.lap_distance_m,
        ]
        .iter()
        .all(|v| v.is_finite())
            && self.game_s >= 0.
            && (0. ..=800.).contains(&self.speed_kmh)
            && self.standings.len() <= 128
            && self.track.len() <= 256
            && self.driver.len() <= 256
            && self.lap >= 0
            && [
                self.fuel_l,
                self.fuel_capacity_l,
                self.ve,
                self.soc,
                self.water_c,
                self.oil_c,
                self.last_lap_s,
                self.session_remaining_s,
            ]
            .iter()
            .flatten()
            .all(|v| v.is_finite() && *v >= 0.)
            && [
                self.rpm,
                self.clutch,
                self.accel[0],
                self.accel[1],
                self.accel[2],
                self.rain,
                self.wetness,
                self.air_c,
                self.track_c,
                self.regen_kw,
                self.brake_bias_front,
                self.lap_start_s,
            ]
            .iter()
            .all(|v| v.is_finite())
            && self.wheels.iter().all(|w| {
                [w.carcass_c, w.pressure_kpa, w.remaining, w.brake_c]
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite())
                    && w.temperature_c
                        .is_none_or(|t| t.iter().all(|v| v.is_finite()))
                    && [w.rotation, w.suspension_m, w.ride_height_m]
                        .iter()
                        .all(|v| v.is_finite())
            })
            && self.standings.iter().all(|v| {
                [v.distance_m, v.x, v.z].iter().all(|v| v.is_finite())
                    && v.driver.len() <= 256
                    && v.vehicle.len() <= 256
            })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub version: u32,
    pub event_id: String,
    pub entry_id: String,
    pub source_id: String,
    pub run_id: String,
    pub seq: u64,
    pub historical: bool,
    pub samples: Vec<Sample>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Hello {
    pub version: u32,
    pub token: String,
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ack {
    pub seq: u64,
    pub accepted: bool,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub start_m: f64,
    pub end_m: f64,
}
pub fn monza_sections() -> Vec<Section> {
    [
        ("Rettifilo", 450., 950.),
        ("Curva Grande", 950., 1750.),
        ("Roggia", 1750., 2250.),
        ("Lesmo 1", 2250., 2550.),
        ("Lesmo 2", 2550., 2850.),
        ("Ascari", 3650., 4250.),
        ("Parabolica", 4600., 5450.),
    ]
    .into_iter()
    .map(|(n, s, e)| Section {
        name: n.into(),
        start_m: s,
        end_m: e,
    })
    .collect()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub event_name: String,
    pub duration_s: f64,
    pub reserve_laps: f64,
    pub reserve_fuel_l: f64,
    pub reserve_ve: f64,
    pub pit_transit_s: f64,
    pub pit_entry_m: Option<f64>,
    pub fuel_l_per_s: f64,
    pub ve_per_s: f64,
    pub tyre_change_s: f64,
    pub driver_change_s: f64,
    pub repair_s: f64,
    pub parallel_service: bool,
    pub extra_lap: bool,
    pub max_stint_s: Option<f64>,
    pub min_drive_s: Option<f64>,
    pub max_drive_s: Option<f64>,
    pub car_plans: std::collections::BTreeMap<String, CarPlan>,
    pub drive_time_adjustments:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    pub tyre_allocation: Option<u32>,
    pub max_tyre_c: f64,
    pub max_brake_c: f64,
    pub confirmed: bool,
    pub sections_track: String,
    pub sections: Vec<Section>,
    pub driver_plan: Vec<DriverPlan>,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            event_name: "Monza · 6 Hours".into(),
            duration_s: 21600.,
            reserve_laps: 1.,
            reserve_fuel_l: 1.,
            reserve_ve: 0.01,
            pit_transit_s: 35.,
            pit_entry_m: None,
            fuel_l_per_s: 2.,
            ve_per_s: 0.02,
            tyre_change_s: 25.,
            driver_change_s: 0.,
            repair_s: 0.,
            parallel_service: false,
            extra_lap: false,
            max_stint_s: None,
            min_drive_s: None,
            max_drive_s: None,
            car_plans: Default::default(),
            drive_time_adjustments: Default::default(),
            tyre_allocation: None,
            max_tyre_c: 115.,
            max_brake_c: 1000.,
            confirmed: false,
            sections_track: String::new(),
            sections: vec![],
            driver_plan: vec![],
        }
    }
}
impl Rules {
    pub fn validate(&self) -> bool {
        [
            self.duration_s,
            self.reserve_laps,
            self.reserve_fuel_l,
            self.reserve_ve,
            self.pit_transit_s,
            self.fuel_l_per_s,
            self.ve_per_s,
            self.tyre_change_s,
            self.driver_change_s,
            self.repair_s,
        ]
        .iter()
        .all(|x| x.is_finite() && *x >= 0.)
            && self.max_tyre_c.is_finite()
            && self.max_tyre_c > 0.
            && self.max_brake_c.is_finite()
            && self.max_brake_c > 0.
            && self
                .pit_entry_m
                .is_none_or(|v| v.is_finite() && v >= 0. && v <= 50000.)
            && self.reserve_ve <= 1.
            && self.reserve_laps <= 100.
            && self
                .min_drive_s
                .zip(self.max_drive_s)
                .is_none_or(|(min, max)| min <= max)
            && self.duration_s > 0.
            && self.duration_s <= 172800.
            && self.fuel_l_per_s > 0.
            && self.ve_per_s > 0.
            && self.sections_track.len() <= 256
            && self.sections.len() <= 100
            && self.sections.iter().all(|s| {
                s.name.len() <= 128
                    && s.start_m.is_finite()
                    && s.end_m.is_finite()
                    && s.start_m >= 0.
                    && s.end_m > s.start_m
            })
            && self.drive_time_adjustments.len() <= 128
            && self.drive_time_adjustments.iter().all(|(entry, drivers)| {
                entry.len() <= 80
                    && drivers.len() <= 32
                    && drivers.iter().all(|(driver, time)| {
                        driver.len() <= 128 && time.is_finite() && time.abs() <= 172800.
                    })
            })
            && self.car_plans.len() <= 128
            && self
                .car_plans
                .iter()
                .all(|(id, p)| id.len() <= 80 && p.validate())
            && self.event_name.len() <= 256
            && self.driver_plan.len() <= 512
            && self.driver_plan.iter().all(|p| {
                p.start_s.is_finite()
                    && p.end_s.is_finite()
                    && p.start_s >= 0.
                    && p.end_s > p.start_s
                    && p.entry_id.len() <= 80
                    && p.driver.len() <= 128
            })
            && self.driver_plan.iter().enumerate().all(|(i, a)| {
                self.driver_plan.iter().skip(i + 1).all(|b| {
                    a.entry_id != b.entry_id || a.end_s <= b.start_s || b.end_s <= a.start_s
                })
            })
            && [self.max_stint_s, self.min_drive_s, self.max_drive_s]
                .iter()
                .flatten()
                .all(|v| v.is_finite() && *v > 0.)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CarPlan {
    pub tyres_used: Option<u32>,
    pub tyre_allocation: Option<u32>,
    pub fuel_l_per_s: Option<f64>,
    pub ve_per_s: Option<f64>,
    pub tyre_change_s: Option<f64>,
    pub driver_change_s: Option<f64>,
    pub pit_transit_s: Option<f64>,
}
impl CarPlan {
    pub fn validate(&self) -> bool {
        [self.fuel_l_per_s, self.ve_per_s]
            .iter()
            .flatten()
            .all(|v| v.is_finite() && *v > 0.)
            && [self.tyre_change_s, self.driver_change_s, self.pit_transit_s]
                .iter()
                .flatten()
                .all(|v| v.is_finite() && *v >= 0.)
    }
    pub fn apply(&self, r: &mut Rules) {
        if let Some(v) = self.fuel_l_per_s {
            r.fuel_l_per_s = v;
        }
        if let Some(v) = self.ve_per_s {
            r.ve_per_s = v;
        }
        if let Some(v) = self.tyre_change_s {
            r.tyre_change_s = v;
        }
        if let Some(v) = self.driver_change_s {
            r.driver_change_s = v;
        }
        if let Some(v) = self.pit_transit_s {
            r.pit_transit_s = v;
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverPlan {
    pub entry_id: String,
    pub driver: String,
    pub start_s: f64,
    pub end_s: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceEvent {
    pub id: String,
    pub at_ms: u64,
    pub game_s: f64,
    pub entry_id: String,
    pub kind: String,
    pub text: String,
    pub manual: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Advice {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub confidence: String,
    pub evidence: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lap {
    pub id: String,
    pub entry_id: String,
    pub driver: String,
    pub vehicle: String,
    pub track: String,
    pub lap: i32,
    pub time_s: f64,
    pub fuel_used_l: Option<f64>,
    pub ve_used: Option<f64>,
    pub valid: bool,
    #[serde(default)]
    pub reference_only: bool,
    pub reason: String,
    pub fuel_start_l: Option<f64>,
    pub rain: f64,
    pub compound: u8,
    pub samples: Vec<Sample>,
}
impl Lap {
    /// Metadata-only copies never clone or retain a trace allocation.
    pub fn metadata(&self) -> Self {
        Self {
            id: self.id.clone(),
            entry_id: self.entry_id.clone(),
            driver: self.driver.clone(),
            vehicle: self.vehicle.clone(),
            track: self.track.clone(),
            lap: self.lap,
            time_s: self.time_s,
            fuel_used_l: self.fuel_used_l,
            ve_used: self.ve_used,
            valid: self.valid,
            reference_only: self.reference_only,
            reason: self.reason.clone(),
            fuel_start_l: self.fuel_start_l,
            rain: self.rain,
            compound: self.compound,
            samples: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Strategy {
    pub preferred_scenario: Option<String>,
    pub clean_laps: usize,
    pub pace_s: Option<f64>,
    pub fuel_per_lap: Option<f64>,
    pub ve_per_lap: Option<f64>,
    pub range_laps: Option<f64>,
    pub limiting: String,
    pub finish_laps: Option<u32>,
    pub stops: Option<u32>,
    pub fuel_target_per_lap: Option<f64>,
    pub ve_target_per_lap: Option<f64>,
    pub fuel_add_l: Option<f64>,
    pub ve_add: Option<f64>,
    pub scenarios: Vec<Scenario>,
    pub assumptions: Vec<String>,
    pub pit_by_lap: Option<i32>,
    pub rejoin_class_position: Option<u32>,
    pub pace_drift_s: Option<f64>,
    pub rivals: Vec<RivalProjection>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RivalProjection {
    pub driver: String,
    pub position: u8,
    pub gap_s: f64,
    pub pace_delta_s: f64,
    pub catch_laps: Option<f64>,
    pub in_pits: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub stops: u32,
    pub finish_s: f64,
    pub fuel_add_l: f64,
    pub ve_add: f64,
    pub feasible: bool,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionComparison {
    pub name: String,
    pub loss_s: f64,
    pub brake_delta_m: Option<f64>,
    pub throttle_delta_m: Option<f64>,
    pub min_speed_delta: f64,
    pub exit_speed_delta: f64,
    pub current_s: f64,
    pub reference_s: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryView {
    pub entry_id: String,
    pub source_id: String,
    pub last_received_ms: u64,
    pub last_sample: Sample,
    pub strategy: Strategy,
    pub advice: Vec<Advice>,
    pub laps: Vec<Lap>,
    pub drive_times: std::collections::BTreeMap<String, f64>,
    pub stint_start_s: f64,
    pub sections: Vec<SectionComparison>,
    pub conflict: bool,
}
