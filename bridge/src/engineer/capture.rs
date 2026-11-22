use super::model::*;
use crate::shared_memory::{
    reader::{SharedMemoryFrame, SharedMemoryReader},
    types::{bytes_to_str, rF2VehicleScoring},
};
pub(crate) fn yellow_flags(
    global: i8,
    sectors: [i8; 3],
    sector: i8,
    individual_phase: u8,
    under_yellow: bool,
) -> bool {
    let index = match sector {
        1 => Some(0),
        2 => Some(1),
        0 => Some(2),
        _ => None,
    };
    global > 0 || under_yellow || individual_phase == 10 || index.is_some_and(|i| sectors[i] == 1)
}

fn positive(v: f64) -> Option<f64> {
    (v.is_finite() && v > 0.).then_some(v)
}
fn fraction(v: f64) -> Option<f64> {
    (v.is_finite() && (0. ..=1.).contains(&v)).then_some(v)
}
fn standing(v: &rF2VehicleScoring) -> Standing {
    Standing {
        slot: v.mID,
        driver: bytes_to_str(&v.mDriverName).into(),
        vehicle: bytes_to_str(&v.mVehicleName).into(),
        class: bytes_to_str(&v.mVehicleClass).into(),
        position: v.mPlace,
        laps: v.mTotalLaps as i32,
        distance_m: v.mLapDist,
        last_lap_s: positive(v.mLastLapTime),
        best_lap_s: positive(v.mBestLapTime),
        gap_s: positive(v.mTimeBehindLeader),
        laps_behind: v.mLapsBehindLeader,
        in_pits: v.mInPits != 0,
        penalties: v.mNumPenalties,
        x: v.mPos.x,
        z: v.mPos.z,
    }
}
pub struct Capture {
    reader: SharedMemoryReader,
    last_game: f64,
    #[cfg(windows)]
    gate: Option<gate::Gate>,
}
impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}
impl Capture {
    pub fn new() -> Self {
        Self {
            reader: SharedMemoryReader::new(),
            last_game: -1.,
            #[cfg(windows)]
            gate: None,
        }
    }
    pub fn sample(&mut self) -> Option<Sample> {
        if !self.reader.is_connected() {
            if !self.reader.open() {
                return None;
            }
            #[cfg(windows)]
            {
                self.gate = gate::Gate::open();
            }
        }
        #[cfg(windows)]
        let _guard = match &self.gate {
            Some(g) => Some(g.acquire()?),
            None => None,
        };
        let frame = self.reader.read_frame()?;
        #[cfg(windows)]
        drop(_guard);
        self.convert_frame(frame)
    }
    fn convert_frame(&mut self, frame: SharedMemoryFrame) -> Option<Sample> {
        let info = &frame.scoring.mScoringInfo;
        if info.mNumVehicles <= 0 || info.mLapDist < 100. {
            return None;
        }
        let tel = frame.telemetry.player();
        let sc = frame.scoring.player();
        let driver_tel = tel.filter(|t| sc.is_some_and(|v| v.mControl == 0 && v.mID == t.mID));
        let game = driver_tel
            .map(|t| t.mElapsedTime)
            .unwrap_or(info.mCurrentET);
        if game == self.last_game {
            return None;
        }
        self.last_game = game;
        let mut s = Sample {
            captured_ms: now_ms(),
            game_s: game,
            track: bytes_to_str(&info.mTrackName).into(),
            track_length_m: info.mLapDist,
            session_kind: info.mSession,
            session_remaining_s: positive(info.mSessionTimeRemaining as f64),
            phase: info.mGamePhase,
            rain: info.mRaining,
            wetness: info.mAvgPathWetness,
            air_c: info.mAmbientTemp,
            track_c: info.mTrackTemp,
            wind: [info.mWind.x, info.mWind.y, info.mWind.z],
            yellow: sc
                .map(|v| {
                    yellow_flags(
                        info.mYellowFlagState,
                        info.mSectorFlag,
                        v.mSector,
                        v.mIndividualPhase,
                        v.mUnderYellow != 0,
                    )
                })
                .unwrap_or(info.mYellowFlagState > 0),
            standings: frame.scoring.mVehicles.iter().map(standing).collect(),
            ..Default::default()
        };
        if let Some(sc) = sc {
            s.slot = Some(sc.mID);
            s.x = sc.mPos.x;
            s.z = sc.mPos.z;
            s.lap_start_s = sc.mLapStartET;
            s.speed_kmh =
                (sc.mLocalVel.x.powi(2) + sc.mLocalVel.y.powi(2) + sc.mLocalVel.z.powi(2)).sqrt()
                    * 3.6;
            s.driver = bytes_to_str(&sc.mDriverName).into();
            s.vehicle = bytes_to_str(&sc.mVehicleName).into();
            s.class = bytes_to_str(&sc.mVehicleClass).into();
            s.lap = sc.mTotalLaps as i32;
            s.lap_distance_m = sc.mLapDist;
            s.in_pits = sc.mInPits != 0;
            s.pit_state = sc.mPitState;
            s.last_lap_s = positive(sc.mLastLapTime);
            s.penalties = sc.mNumPenalties;
            if let Some(t) = driver_tel {
                s.slot = Some(t.mID);
                s.authoritative = true;
                s.valid_lap = t.mLapInvalidated == 0 && sc.mCountLapFlag == 2;
                // Keep the clock coherent with scoring's lap number/distance.
                // Telemetry can roll to the next lap before the 5 Hz scoring update.
                s.speed_kmh =
                    (t.mLocalVel.x.powi(2) + t.mLocalVel.y.powi(2) + t.mLocalVel.z.powi(2)).sqrt()
                        * 3.6;
                // Scoring distance is 5 Hz. Extrapolate by at most one scoring tick, never across S/F.
                s.lap_distance_m = (sc.mLapDist
                    + (game - info.mCurrentET).clamp(0., 0.2) * s.speed_kmh / 3.6)
                    .min(info.mLapDist - 0.001);
                s.rpm = t.mEngineRPM;
                s.gear = t.mGear;
                s.throttle = t.mFilteredThrottle;
                s.brake = t.mFilteredBrake;
                s.steering = t.mFilteredSteering;
                s.clutch = t.mFilteredClutch;
                s.x = t.mPos.x;
                s.z = t.mPos.z;
                s.accel = [t.mLocalAccel.x, t.mLocalAccel.y, t.mLocalAccel.z];
                s.fuel_l =
                    (t.mFuel.is_finite() && t.mFuel >= 0. && t.mFuel <= 1000.).then_some(t.mFuel);
                s.fuel_capacity_l = positive(t.mFuelCapacity);
                s.ve = fraction(t.mVirtualEnergy as f64);
                s.soc = fraction(t.mBatteryChargeFraction);
                s.water_c = positive(t.mEngineWaterTemp);
                s.oil_c = positive(t.mEngineOilTemp);
                s.tc = t.mTC;
                s.abs = t.mABS;
                s.tc_active = t.mTCActive != 0;
                s.abs_active = t.mABSActive != 0;
                s.brake_bias_front = 1. - t.mRearBrakeBias;
                s.motor_map = t.mMotorMap;
                s.migration = t.mMigration;
                s.regen_kw = t.mRegen as f64;
                s.track_limits = t.mTrackLimitsSteps;
                s.impact = t.mLastImpactMagnitude;
                s.impact_s = t.mLastImpactET;
                s.dents = t.mDentSeverity;
                s.detached = t.mDetached != 0;
                s.wheels = std::array::from_fn(|i| {
                    let w = t.mWheels[i];
                    let temps = w.mTemperature;
                    Wheel {
                        temperature_c: temps
                            .iter()
                            .all(|v| v.is_finite() && *v > 150. && *v < 600.)
                            .then(|| temps.map(|v| v - 273.15)),
                        carcass_c: positive(w.mTireCarcassTemperature).map(|v| v - 273.15),
                        pressure_kpa: positive(w.mPressure),
                        remaining: fraction(w.mWear),
                        brake_c: positive(w.mBrakeTemp).map(|v| v - 273.15),
                        detached: w.mDetached != 0,
                        compound: w.mCompoundIndex,
                        suspension_m: w.mSuspensionDeflection,
                        ride_height_m: w.mRideHeight,
                        rotation: w.mRotation,
                    }
                });
            }
        }
        s.validate().then_some(s)
    }
    pub fn reconnect(&mut self) {
        self.reader.close();
        // A paused or abandoned mapping can still exist while another reader
        // holds it. Reopening it must not turn its frozen frame into new data.
        #[cfg(windows)]
        {
            self.gate = None;
        }
    }
}

#[cfg(windows)]
mod gate {
    use std::sync::atomic::{AtomicI32, Ordering};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Memory::{
                FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, OpenFileMappingA,
                UnmapViewOfFile,
            },
            Threading::{OpenEventA, SetEvent},
        },
    };
    pub struct Gate {
        handle: HANDLE,
        ptr: *mut u8,
        event: HANDLE,
    }
    unsafe impl Send for Gate {}
    impl Gate {
        pub fn open() -> Option<Self> {
            unsafe {
                let h = OpenFileMappingA(
                    FILE_MAP_ALL_ACCESS,
                    0,
                    c"LMU_SharedMemoryLockData".as_ptr() as _,
                );
                if h.is_null() {
                    return None;
                }
                let p = MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, 8);
                if p.Value.is_null() {
                    CloseHandle(h);
                    return None;
                }
                let event = OpenEventA(2, 0, c"LMU_SharedMemoryLockEvent".as_ptr() as _);
                Some(Self {
                    handle: h,
                    ptr: p.Value as _,
                    event,
                })
            }
        }
        pub fn acquire(&self) -> Option<Guard<'_>> {
            let busy = unsafe { &*(self.ptr.add(4) as *const AtomicI32) };
            for _ in 0..64 {
                if busy
                    .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return Some(Guard(self));
                }
                std::hint::spin_loop()
            }
            None
        }
    }
    pub struct Guard<'a>(&'a Gate);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            unsafe {
                (&*(self.0.ptr.add(4) as *const AtomicI32)).store(0, Ordering::Release);
                if (&*(self.0.ptr as *const AtomicI32)).load(Ordering::Relaxed) > 0
                    && !self.0.event.is_null()
                {
                    SetEvent(self.0.event);
                }
            }
        }
    }
    impl Drop for Gate {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as _,
                });
                CloseHandle(self.handle);
                if !self.event.is_null() {
                    CloseHandle(self.event);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reopening_a_frozen_mapping_does_not_refresh_telemetry() {
        use crate::shared_memory::{reader::{ScoringFrame, TelemetryFrame}, types::*};
        let mut info: rF2ScoringInfo = unsafe { std::mem::zeroed() };
        info.mNumVehicles = 1;
        info.mLapDist = 5412.;
        info.mCurrentET = 10.;
        info.mTrackName[..7].copy_from_slice(b"Bahrain");
        let mut car: rF2VehicleScoring = unsafe { std::mem::zeroed() };
        car.mID = 7;
        car.mIsPlayer = 1;
        car.mDriverName[0] = b'A';
        car.mVehicleName[0] = b'G';
        car.mLapDist = 100.;
        let mut telemetry: rF2VehicleTelemetry = unsafe { std::mem::zeroed() };
        telemetry.mID = 7;
        telemetry.mElapsedTime = 10.;
        telemetry.mFuel = 70.;
        telemetry.mFuelCapacity = 100.;
        let mut frame = super::SharedMemoryFrame {
            scoring: ScoringFrame { mScoringInfo: info, mVehicles: vec![car] },
            telemetry: TelemetryFrame { mVehicles: vec![telemetry], player_idx: Some(0) },
        };
        let mut capture = super::Capture::new();
        assert_eq!(capture.convert_frame(frame.clone()).unwrap().game_s, 10.);
        for _ in 0..3 {
            capture.reconnect();
            assert!(capture.convert_frame(frame.clone()).is_none());
        }
        frame.telemetry.mVehicles[0].mElapsedTime = 10.02;
        assert_eq!(capture.convert_frame(frame.clone()).unwrap().game_s, 10.02);
        capture.reconnect();
        frame.telemetry.mVehicles[0].mElapsedTime = 0.;
        assert_eq!(capture.convert_frame(frame).unwrap().game_s, 0.);
    }
    #[test]
    fn native_clear_sector_is_not_a_yellow() {
        assert!(!super::yellow_flags(0, [11; 3], 1, 5, false));
        assert!(super::yellow_flags(0, [11, 1, 11], 2, 5, false));
        assert!(super::yellow_flags(2, [11; 3], 1, 5, false));
        assert!(!super::yellow_flags(0, [11, 11, 1], 1, 5, false));
        assert!(super::yellow_flags(0, [11; 3], 1, 10, false));
    }
}
