//! Independently versioned rF2 SMMP 3.7 telemetry layout.
//! Field definitions follow TheIronWolfModding/rF2SharedMemoryMapPlugin Include/rF2State.h.
//! LMU additions carved into reserved bytes are intentionally NOT interpreted here.
#![allow(non_snake_case)]
use crate::shared_memory::types::rF2Vec3;
#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct LegacyWheel {
    pub mSuspensionDeflection: f64, // meters
    pub mRideHeight: f64,           // meters
    pub mSuspForce: f64,            // pushrod load in Newtons
    pub mBrakeTemp: f64,            // Kelvin. Both the rF2 SDK header and LMU's own
    // SharedMemoryInterface.hpp comment this as "Celsius",
    // but the value is Kelvin in practice — main.rs
    // subtracts 273.15 and the dashboard's °C thresholds
    // only make sense that way. See BRAKE_TEMP_K_* below.
    // Recheck if LMU ever fixes the header comment.
    pub mBrakePressure: f64, // 0.0-1.0 (future: true kPa)

    pub mRotation: f64,              // radians/sec
    pub mLateralPatchVel: f64,       // m/s lateral velocity at contact patch
    pub mLongitudinalPatchVel: f64,  // m/s longitudinal velocity at contact patch
    pub mLateralGroundVel: f64,      // m/s lateral ground velocity
    pub mLongitudinalGroundVel: f64, // m/s longitudinal ground velocity
    pub mCamber: f64,                // radians
    pub mLateralForce: f64,          // Newtons
    pub mLongitudinalForce: f64,     // Newtons
    pub mTireLoad: f64,              // Newtons

    pub mGripFract: f64,              // fraction of patch that is sliding (0-1)
    pub mPressure: f64,               // kPa tire air pressure
    pub mTemperature: [f64; 3],       // Kelvin: left/center/right
    pub mWear: f64, // 1.0 (new) – 0.0 (destroyed) [LMU convention; inverted vs. rF2 plugin docs]
    pub mTerrainName: [u8; 16], // material prefix from TDF
    pub mSurfaceType: u8, // 0=dry,1=wet,2=grass,3=dirt,4=gravel,5=rumble,6=special
    pub mFlat: u8,  // 1 if flat
    pub mDetached: u8, // 1 if detached
    pub mStaticUndeflectedRadius: u8, // tire radius in centimetres

    pub mVerticalTireDeflection: f64, // deflection from static radius
    pub mWheelYLocation: f64,         // wheel Y relative to vehicle Y
    pub mToe: f64,                    // current toe angle (rad) w.r.t. vehicle

    pub mTireCarcassTemperature: f64, // rough average carcass temperature (Kelvin)
    pub mTireInnerLayerTemperature: [f64; 3], // rough average inner-layer temperatures (Kelvin)

    pub expansion: [u8; 24],
}

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct LegacyTelemetry {
    // --- identification / timing ---
    pub mID: i32,          // slot ID (may change post-init)
    pub mDeltaTime: f64,   // seconds since last update
    pub mElapsedTime: f64, // game session time (seconds)
    pub mLapNumber: i32,   // current lap (0-based)
    pub mLapStartET: f64,  // elapsed time when this lap started

    pub mVehicleName: [u8; 64], // vehicle model name
    pub mTrackName: [u8; 64],   // track name

    // --- position / kinematics ---
    pub mPos: rF2Vec3,           // world position (metres)
    pub mLocalVel: rF2Vec3,      // velocity in local coords (m/s)
    pub mLocalAccel: rF2Vec3,    // acceleration in local coords (m/s²)
    pub mOri: [rF2Vec3; 3],      // orientation matrix rows (local→world)
    pub mLocalRot: rF2Vec3,      // angular velocity in local coords (rad/s)
    pub mLocalRotAccel: rF2Vec3, // angular acceleration in local coords (rad/s²)

    // --- powertrain / controls ---
    pub mGear: i32,            // -1=reverse, 0=neutral, 1+=forward
    pub mEngineRPM: f64,       // current RPM  ← BEFORE mEngineMaxRPM
    pub mEngineWaterTemp: f64, // Celsius
    pub mEngineOilTemp: f64,   // Celsius
    pub mClutchRPM: f64,       // clutch-side RPM

    // raw (unfiltered) driver inputs
    pub mUnfilteredThrottle: f64, // 0.0 – 1.0
    pub mUnfilteredBrake: f64,    // 0.0 – 1.0
    pub mUnfilteredSteering: f64, // -1.0 (left) – 1.0 (right)
    pub mUnfilteredClutch: f64,   // 0.0 – 1.0

    // filtered / corrected inputs
    pub mFilteredThrottle: f64, // 0.0 – 1.0
    pub mFilteredBrake: f64,    // 0.0 – 1.0
    pub mFilteredSteering: f64, // -1.0 – 1.0
    pub mFilteredClutch: f64,   // 0.0 – 1.0

    // --- chassis / aero ---
    pub mSteeringShaftTorque: f64, // Nm (useful for FFB)
    pub mFront3rdDeflection: f64,  // front third-spring deflection (m)
    pub mRear3rdDeflection: f64,   // rear third-spring deflection (m)

    pub mFrontWingHeight: f64, // front wing height (m)
    pub mFrontRideHeight: f64, // front ride height (m)
    pub mRearRideHeight: f64,  // rear ride height (m)
    pub mDrag: f64,            // drag force (Newtons)
    pub mFrontDownforce: f64,  // front downforce (Newtons)
    pub mRearDownforce: f64,   // rear downforce (Newtons)

    // --- state / fuel ---
    pub mFuel: f64,         // current fuel level (litres)
    pub mEngineMaxRPM: f64, // redline RPM  ← AFTER mFuel

    // --- vehicle state flags ---
    pub mScheduledStops: u8,    // scheduled pit stops remaining
    pub mOverheating: u8,       // 1 if overheating icon shown
    pub mDetached: u8,          // 1 if any part is detached
    pub mHeadlights: u8,        // 1 if headlights on
    pub mDentSeverity: [u8; 8], // 0=none,1=dented,2=very dented (8 locations)

    pub mLastImpactET: f64,        // session time of last impact
    pub mLastImpactMagnitude: f64, // magnitude of last impact
    pub mLastImpactPos: rF2Vec3,   // world position of last impact

    // --- expansion block (formerly "Expanded") ---
    pub mEngineTorque: f64,               // engine torque at wheels (Nm)
    pub mCurrentSector: i32,              // 0=S1, 1=S2, 2=S3 (sign bit = in pitlane)
    pub mSpeedLimiter: u8,                // 1 if pit speed limiter active
    pub mMaxGears: u8,                    // number of forward gears
    pub mFrontTireCompoundIndex: u8,      // compound index (front)
    pub mRearTireCompoundIndex: u8,       // compound index (rear)
    pub mFuelCapacity: f64,               // tank capacity (litres)
    pub mFrontFlapActivated: u8,          // 1 if front flap activated
    pub mRearFlapActivated: u8,           // 1 if rear flap activated
    pub mRearFlapLegalStatus: u8,         // 0=disallowed,1=detected/pending,2=allowed
    pub mIgnitionStarter: u8,             // 0=off,1=ignition,2=ignition+starter
    pub mFrontTireCompoundName: [u8; 18], // compound name (front)
    pub mRearTireCompoundName: [u8; 18],  // compound name (rear)
    pub mSpeedLimiterAvailable: u8,       // 1 if speed limiter available
    pub mAntiStallActivated: u8,          // 1 if anti-stall active
    pub mUnused: [u8; 2],
    pub mVisualSteeringWheelRange: f32, // visual lock-to-lock range (degrees)
    pub mRearBrakeBias: f64,            // rear brake bias fraction
    pub mTurboBoostPressure: f64,       // turbo boost (bar)
    pub mPhysicsToGraphicsOffset: [f32; 3], // physics→graphics centre offset
    pub mPhysicalSteeringWheelRange: f32, // physical lock-to-lock range (degrees)
    pub mBatteryChargeFraction: f64,
    pub mElectricBoostMotorTorque: f64,
    pub mElectricBoostMotorRPM: f64,
    pub mElectricBoostMotorTemperature: f64,
    pub mElectricBoostWaterTemperature: f64,
    pub mElectricBoostMotorState: u8,
    pub expansion: [u8; 111],
    pub mWheels: [LegacyWheel; 4],
}

const _: () = assert!(std::mem::size_of::<LegacyWheel>() == 260);
const _: () = assert!(std::mem::size_of::<LegacyTelemetry>() == 1888);
