//! Legacy rF2 SMMP reader. Reads no DMA or game-control buffers.
use super::{legacy_types::*, model::*};
#[cfg(windows)]
mod platform {
    use super::*;
    use crate::shared_memory::types::{bytes_to_str, rF2ScoringInfo, rF2VehicleScoring};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Memory::{
            FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingA,
            UnmapViewOfFile,
        },
    };
    struct Buffer {
        handle: HANDLE,
        ptr: *mut u8,
    }
    impl Buffer {
        fn open(name: &str, size: usize) -> Option<Self> {
            let name = std::ffi::CString::new(name).ok()?;
            unsafe {
                let h = OpenFileMappingA(FILE_MAP_READ, 0, name.as_ptr() as _);
                if h.is_null() {
                    return None;
                }
                let p = MapViewOfFile(h, FILE_MAP_READ, 0, 0, size);
                if p.Value.is_null() {
                    CloseHandle(h);
                    return None;
                }
                Some(Self {
                    handle: h,
                    ptr: p.Value as _,
                })
            }
        }
        unsafe fn at<T: Copy>(&self, offset: usize) -> T {
            std::ptr::read_unaligned(self.ptr.add(offset) as *const T)
        }
        fn copy<T: Copy>(&self, offset: usize) -> Option<T> {
            for _ in 0..5 {
                unsafe {
                    let begin = self.at::<u32>(0);
                    let end = self.at::<u32>(4);
                    if begin != end {
                        continue;
                    }
                    let value = self.at(offset);
                    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
                    if begin == self.at::<u32>(0) && end == self.at::<u32>(4) {
                        return Some(value);
                    }
                }
            }
            None
        }
    }
    impl Drop for Buffer {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as _,
                });
                CloseHandle(self.handle);
            }
        }
    }
    #[repr(C, packed(4))]
    #[derive(Clone, Copy)]
    struct LegacyScoring {
        begin: u32,
        end: u32,
        hint: i32,
        info: rF2ScoringInfo,
        cars: [rF2VehicleScoring; 128],
    }
    #[repr(C, packed(4))]
    #[derive(Clone, Copy)]
    struct LegacyTelemBuffer {
        begin: u32,
        end: u32,
        hint: i32,
        count: i32,
        cars: [LegacyTelemetry; 128],
    }
    pub struct LegacyCapture {
        scoring: Option<Buffer>,
        telemetry: Option<Buffer>,
        last: f64,
    }
    impl LegacyCapture {
        pub fn new() -> Self {
            Self {
                scoring: None,
                telemetry: None,
                last: -1.,
            }
        }
        pub fn sample(&mut self) -> Option<Sample> {
            if self.scoring.is_none() {
                let extended = Buffer::open("$rFactor2SMMP_Extended$", 24)?;
                let version = extended.copy::<[u8; 12]>(8)?;
                if !bytes_to_str(&version).starts_with("3.7.") {
                    return None;
                }
                self.scoring = Buffer::open(
                    "$rFactor2SMMP_Scoring$",
                    std::mem::size_of::<LegacyScoring>(),
                );
                self.telemetry = Buffer::open(
                    "$rFactor2SMMP_Telemetry$",
                    std::mem::size_of::<LegacyTelemBuffer>(),
                );
            }
            let sc = self.scoring.as_ref()?.copy::<LegacyScoring>(0)?;
            let tel = self.telemetry.as_ref()?.copy::<LegacyTelemBuffer>(0)?;
            let count = sc.info.mNumVehicles;
            if !(1..=128).contains(&count) || !(0..=128).contains(&tel.count) {
                return None;
            }
            let rows = &sc.cars[..count as usize];
            let v = rows.iter().find(|v| v.mIsPlayer != 0 && v.mControl == 0)?;
            let id = v.mID;
            let t = tel.cars[..tel.count as usize]
                .iter()
                .find(|t| t.mID == id)?;
            let et = t.mElapsedTime;
            if et == self.last {
                return None;
            }
            self.last = et;
            let f = |v: f64| {
                if v.is_finite() && v >= 0. {
                    Some(v)
                } else {
                    None
                }
            };
            let s = Sample {
                captured_ms: now_ms(),
                game_s: et,
                track: bytes_to_str(&sc.info.mTrackName).into(),
                track_length_m: sc.info.mLapDist,
                session_kind: sc.info.mSession,
                session_remaining_s: f(sc.info.mEndET - et),
                phase: sc.info.mGamePhase,
                slot: Some(id),
                driver: bytes_to_str(&v.mDriverName).into(),
                vehicle: bytes_to_str(&v.mVehicleName).into(),
                class: bytes_to_str(&v.mVehicleClass).into(),
                lap: v.mTotalLaps as i32,
                lap_distance_m: v.mLapDist,
                lap_start_s: v.mLapStartET,
                last_lap_s: f(v.mLastLapTime).filter(|v| *v > 0.),
                authoritative: true,
                valid_lap: v.mCountLapFlag == 2,
                in_pits: v.mInPits != 0,
                pit_state: v.mPitState,
                speed_kmh: (t.mLocalVel.x.powi(2) + t.mLocalVel.y.powi(2) + t.mLocalVel.z.powi(2))
                    .sqrt()
                    * 3.6,
                rpm: t.mEngineRPM,
                gear: t.mGear,
                throttle: t.mFilteredThrottle,
                brake: t.mFilteredBrake,
                steering: t.mFilteredSteering,
                clutch: t.mFilteredClutch,
                x: t.mPos.x,
                z: t.mPos.z,
                accel: [t.mLocalAccel.x, t.mLocalAccel.y, t.mLocalAccel.z],
                fuel_l: f(t.mFuel),
                fuel_capacity_l: f(t.mFuelCapacity),
                // The LMU legacy buffer reports a placeholder here; use the
                // native interface for battery state rather than showing 0%.
                soc: None,
                water_c: f(t.mEngineWaterTemp),
                oil_c: f(t.mEngineOilTemp),
                wheels: std::array::from_fn(|i| {
                    let w = t.mWheels[i];
                    let temps = w.mTemperature;
                    Wheel {
                        temperature_c: temps
                            .iter()
                            .all(|t| t.is_finite() && *t > 150. && *t < 600.)
                            .then(|| temps.map(|v| v - 273.15)),
                        carcass_c: f(w.mTireCarcassTemperature).map(|v| v - 273.15),
                        pressure_kpa: f(w.mPressure),
                        remaining: f(w.mWear).filter(|v| *v <= 1.),
                        brake_c: f(w.mBrakeTemp).map(|v| v - 273.15),
                        detached: w.mDetached != 0,
                        compound: if i < 2 {
                            t.mFrontTireCompoundIndex
                        } else {
                            t.mRearTireCompoundIndex
                        },
                        suspension_m: w.mSuspensionDeflection,
                        ride_height_m: w.mRideHeight,
                        rotation: w.mRotation,
                    }
                }),
                brake_bias_front: 1. - t.mRearBrakeBias,
                impact: t.mLastImpactMagnitude,
                impact_s: t.mLastImpactET,
                dents: t.mDentSeverity,
                detached: t.mDetached != 0,
                rain: sc.info.mRaining,
                wetness: sc.info.mAvgPathWetness,
                air_c: sc.info.mAmbientTemp,
                track_c: sc.info.mTrackTemp,
                yellow: super::super::capture::yellow_flags(
                    sc.info.mYellowFlagState,
                    sc.info.mSectorFlag,
                    v.mSector,
                    v.mIndividualPhase,
                    v.mUnderYellow != 0,
                ),
                penalties: v.mNumPenalties,
                standings: rows
                    .iter()
                    .map(|v| Standing {
                        slot: v.mID,
                        driver: bytes_to_str(&v.mDriverName).into(),
                        vehicle: bytes_to_str(&v.mVehicleName).into(),
                        class: bytes_to_str(&v.mVehicleClass).into(),
                        position: v.mPlace,
                        laps: v.mTotalLaps as i32,
                        distance_m: v.mLapDist,
                        last_lap_s: f(v.mLastLapTime).filter(|v| *v > 0.),
                        best_lap_s: f(v.mBestLapTime).filter(|v| *v > 0.),
                        gap_s: f(v.mTimeBehindLeader).filter(|gap| *gap > 0.),
                        laps_behind: v.mLapsBehindLeader,
                        in_pits: v.mInPits != 0,
                        penalties: v.mNumPenalties,
                        x: v.mPos.x,
                        z: v.mPos.z,
                    })
                    .collect(),
                supplemental: Some(
                    serde_json::json!({"adapter":"rF2 SMMP 3.7","capabilities":{"virtual_energy":false,"battery_soc":false,"native_electronics":false}}),
                ),
                ..Default::default()
            };
            s.validate().then_some(s)
        }
        pub fn reconnect(&mut self) {
            self.scoring = None;
            self.telemetry = None;
            // Retain the last observed clock so a paused/abandoned mapping
            // cannot refresh the feed merely because its handles reopened.
        }
    }
}
#[cfg(windows)]
pub use platform::LegacyCapture;
#[cfg(not(windows))]
pub struct LegacyCapture;
#[cfg(not(windows))]
impl LegacyCapture {
    pub fn new() -> Self {
        Self
    }
    pub fn sample(&mut self) -> Option<Sample> {
        None
    }
    pub fn reconnect(&mut self) {}
}
