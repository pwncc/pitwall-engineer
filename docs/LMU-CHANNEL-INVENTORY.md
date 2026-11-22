# LMU field inventory

Generated from the MIT-licensed Pitwall Rust mapping and the separate legacy adapter. Field names describe the wire contract; presence in this list does not mean a channel is available for every car or is displayed in the engineer UI. Installed SDK sizes were checked with C++ on 21 November 2026. Proprietary SDK headers are not redistributed.

Native telemetry: 1888 bytes; wheel: 260; scoring: 548; vehicle scoring: 584. Outer object: 324824 bytes, including four tail alignment bytes.

## bridge/src/shared_memory/types.rs

| Record | Field | Rust representation | Mapping note |
|---|---|---|---|
| rF2Vec3 | `x` | `f64` |  |
| rF2Vec3 | `y` | `f64` |  |
| rF2Vec3 | `z` | `f64` |  |
| rF2Wheel | `mSuspensionDeflection` | `f64` | meters |
| rF2Wheel | `mRideHeight` | `f64` | meters |
| rF2Wheel | `mSuspForce` | `f64` | pushrod load in Newtons |
| rF2Wheel | `mBrakeTemp` | `f64` | Kelvin. Both the rF2 SDK header and LMU's own |
| rF2Wheel | `mBrakePressure` | `f64` | 0.0-1.0 (future: true kPa) |
| rF2Wheel | `mRotation` | `f64` | radians/sec |
| rF2Wheel | `mLateralPatchVel` | `f64` | m/s lateral velocity at contact patch |
| rF2Wheel | `mLongitudinalPatchVel` | `f64` | m/s longitudinal velocity at contact patch |
| rF2Wheel | `mLateralGroundVel` | `f64` | m/s lateral ground velocity |
| rF2Wheel | `mLongitudinalGroundVel` | `f64` | m/s longitudinal ground velocity |
| rF2Wheel | `mCamber` | `f64` | radians |
| rF2Wheel | `mLateralForce` | `f64` | Newtons |
| rF2Wheel | `mLongitudinalForce` | `f64` | Newtons |
| rF2Wheel | `mTireLoad` | `f64` | Newtons |
| rF2Wheel | `mGripFract` | `f64` | fraction of patch that is sliding (0-1) |
| rF2Wheel | `mPressure` | `f64` | kPa tire air pressure |
| rF2Wheel | `mTemperature` | `[f64; 3]` | Kelvin: left/center/right |
| rF2Wheel | `mWear` | `f64` | 1.0 (new) – 0.0 (destroyed) [LMU convention; inverted vs. rF2 plugin docs] |
| rF2Wheel | `mTerrainName` | `[u8; 16]` | material prefix from TDF |
| rF2Wheel | `mSurfaceType` | `u8` | 0=dry,1=wet,2=grass,3=dirt,4=gravel,5=rumble,6=special |
| rF2Wheel | `mFlat` | `u8` | 1 if flat |
| rF2Wheel | `mDetached` | `u8` | 1 if detached |
| rF2Wheel | `mStaticUndeflectedRadius` | `u8` | tire radius in centimetres |
| rF2Wheel | `mVerticalTireDeflection` | `f64` | deflection from static radius |
| rF2Wheel | `mWheelYLocation` | `f64` | wheel Y relative to vehicle Y |
| rF2Wheel | `mToe` | `f64` | current toe angle (rad) w.r.t. vehicle |
| rF2Wheel | `mTireCarcassTemperature` | `f64` | rough average carcass temperature (Kelvin) |
| rF2Wheel | `mTireInnerLayerTemperature` | `[f64; 3]` | rough average inner-layer temperatures (Kelvin) |
| rF2Wheel | `mOptimalTemp` | `f32` | optimal tire temperature |
| rF2Wheel | `mCompoundIndex` | `u8` | compound index |
| rF2Wheel | `mCompoundType` | `u8` | compound type |
| rF2Wheel | `mExpansion` | `[u8; 18]` | reserved for future use (was 24) |
| rF2VehicleTelemetry | `mID` | `i32` | slot ID (may change post-init) |
| rF2VehicleTelemetry | `mDeltaTime` | `f64` | seconds since last update |
| rF2VehicleTelemetry | `mElapsedTime` | `f64` | game session time (seconds) |
| rF2VehicleTelemetry | `mLapNumber` | `i32` | current lap (0-based) |
| rF2VehicleTelemetry | `mLapStartET` | `f64` | elapsed time when this lap started |
| rF2VehicleTelemetry | `mVehicleName` | `[u8; 64]` | vehicle model name |
| rF2VehicleTelemetry | `mTrackName` | `[u8; 64]` | track name |
| rF2VehicleTelemetry | `mPos` | `rF2Vec3` | world position (metres) |
| rF2VehicleTelemetry | `mLocalVel` | `rF2Vec3` | velocity in local coords (m/s) |
| rF2VehicleTelemetry | `mLocalAccel` | `rF2Vec3` | acceleration in local coords (m/s²) |
| rF2VehicleTelemetry | `mOri` | `[rF2Vec3; 3]` | orientation matrix rows (local→world) |
| rF2VehicleTelemetry | `mLocalRot` | `rF2Vec3` | angular velocity in local coords (rad/s) |
| rF2VehicleTelemetry | `mLocalRotAccel` | `rF2Vec3` | angular acceleration in local coords (rad/s²) |
| rF2VehicleTelemetry | `mGear` | `i32` | -1=reverse, 0=neutral, 1+=forward |
| rF2VehicleTelemetry | `mEngineRPM` | `f64` | current RPM  ← BEFORE mEngineMaxRPM |
| rF2VehicleTelemetry | `mEngineWaterTemp` | `f64` | Celsius |
| rF2VehicleTelemetry | `mEngineOilTemp` | `f64` | Celsius |
| rF2VehicleTelemetry | `mClutchRPM` | `f64` | clutch-side RPM |
| rF2VehicleTelemetry | `mUnfilteredThrottle` | `f64` | 0.0 – 1.0 |
| rF2VehicleTelemetry | `mUnfilteredBrake` | `f64` | 0.0 – 1.0 |
| rF2VehicleTelemetry | `mUnfilteredSteering` | `f64` | -1.0 (left) – 1.0 (right) |
| rF2VehicleTelemetry | `mUnfilteredClutch` | `f64` | 0.0 – 1.0 |
| rF2VehicleTelemetry | `mFilteredThrottle` | `f64` | 0.0 – 1.0 |
| rF2VehicleTelemetry | `mFilteredBrake` | `f64` | 0.0 – 1.0 |
| rF2VehicleTelemetry | `mFilteredSteering` | `f64` | -1.0 – 1.0 |
| rF2VehicleTelemetry | `mFilteredClutch` | `f64` | 0.0 – 1.0 |
| rF2VehicleTelemetry | `mSteeringShaftTorque` | `f64` | Nm (useful for FFB) |
| rF2VehicleTelemetry | `mFront3rdDeflection` | `f64` | front third-spring deflection (m) |
| rF2VehicleTelemetry | `mRear3rdDeflection` | `f64` | rear third-spring deflection (m) |
| rF2VehicleTelemetry | `mFrontWingHeight` | `f64` | front wing height (m) |
| rF2VehicleTelemetry | `mFrontRideHeight` | `f64` | front ride height (m) |
| rF2VehicleTelemetry | `mRearRideHeight` | `f64` | rear ride height (m) |
| rF2VehicleTelemetry | `mDrag` | `f64` | drag force (Newtons) |
| rF2VehicleTelemetry | `mFrontDownforce` | `f64` | front downforce (Newtons) |
| rF2VehicleTelemetry | `mRearDownforce` | `f64` | rear downforce (Newtons) |
| rF2VehicleTelemetry | `mFuel` | `f64` | current fuel level (litres) |
| rF2VehicleTelemetry | `mEngineMaxRPM` | `f64` | redline RPM  ← AFTER mFuel |
| rF2VehicleTelemetry | `mScheduledStops` | `u8` | scheduled pit stops remaining |
| rF2VehicleTelemetry | `mOverheating` | `u8` | 1 if overheating icon shown |
| rF2VehicleTelemetry | `mDetached` | `u8` | 1 if any part is detached |
| rF2VehicleTelemetry | `mHeadlights` | `u8` | 1 if headlights on |
| rF2VehicleTelemetry | `mDentSeverity` | `[u8; 8]` | 0=none,1=dented,2=very dented (8 locations) |
| rF2VehicleTelemetry | `mLastImpactET` | `f64` | session time of last impact |
| rF2VehicleTelemetry | `mLastImpactMagnitude` | `f64` | magnitude of last impact |
| rF2VehicleTelemetry | `mLastImpactPos` | `rF2Vec3` | world position of last impact |
| rF2VehicleTelemetry | `mEngineTorque` | `f64` | engine torque at wheels (Nm) |
| rF2VehicleTelemetry | `mCurrentSector` | `i32` | 0=S1, 1=S2, 2=S3 (sign bit = in pitlane) |
| rF2VehicleTelemetry | `mSpeedLimiter` | `u8` | 1 if pit speed limiter active |
| rF2VehicleTelemetry | `mMaxGears` | `u8` | number of forward gears |
| rF2VehicleTelemetry | `mFrontTireCompoundIndex` | `u8` | compound index (front) |
| rF2VehicleTelemetry | `mRearTireCompoundIndex` | `u8` | compound index (rear) |
| rF2VehicleTelemetry | `mFuelCapacity` | `f64` | tank capacity (litres) |
| rF2VehicleTelemetry | `mFrontFlapActivated` | `u8` | 1 if front flap activated |
| rF2VehicleTelemetry | `mRearFlapActivated` | `u8` | 1 if rear flap activated |
| rF2VehicleTelemetry | `mRearFlapLegalStatus` | `u8` | 0=disallowed,1=detected/pending,2=allowed |
| rF2VehicleTelemetry | `mIgnitionStarter` | `u8` | 0=off,1=ignition,2=ignition+starter |
| rF2VehicleTelemetry | `mFrontTireCompoundName` | `[u8; 18]` | compound name (front) |
| rF2VehicleTelemetry | `mRearTireCompoundName` | `[u8; 18]` | compound name (rear) |
| rF2VehicleTelemetry | `mSpeedLimiterAvailable` | `u8` | 1 if speed limiter available |
| rF2VehicleTelemetry | `mAntiStallActivated` | `u8` | 1 if anti-stall active |
| rF2VehicleTelemetry | `mUnused` | `[u8; 2]` |  |
| rF2VehicleTelemetry | `mVisualSteeringWheelRange` | `f32` | visual lock-to-lock range (degrees) |
| rF2VehicleTelemetry | `mRearBrakeBias` | `f64` | rear brake bias fraction |
| rF2VehicleTelemetry | `mTurboBoostPressure` | `f64` | turbo boost (bar) |
| rF2VehicleTelemetry | `mPhysicsToGraphicsOffset` | `[f32; 3]` | physics→graphics centre offset |
| rF2VehicleTelemetry | `mPhysicalSteeringWheelRange` | `f32` | physical lock-to-lock range (degrees) |
| rF2VehicleTelemetry | `mDeltaBest` | `f64` | delta to personal best lap (seconds) |
| rF2VehicleTelemetry | `mBatteryChargeFraction` | `f64` | battery charge [0.0-1.0] |
| rF2VehicleTelemetry | `mElectricBoostMotorTorque` | `f64` | boost motor torque (Nm; negative = regen) |
| rF2VehicleTelemetry | `mElectricBoostMotorRPM` | `f64` | boost motor RPM |
| rF2VehicleTelemetry | `mElectricBoostMotorTemperature` | `f64` | boost motor temperature (Celsius) |
| rF2VehicleTelemetry | `mElectricBoostWaterTemperature` | `f64` | boost motor coolant temperature (Celsius; 0 if absent) |
| rF2VehicleTelemetry | `mElectricBoostMotorState` | `u8` | 0=unavailable,1=inactive,2=propulsion,3=regen |
| rF2VehicleTelemetry | `mLapInvalidated` | `u8` | 1 if current lap invalidated |
| rF2VehicleTelemetry | `mABSActive` | `u8` | 1 if ABS currently intervening |
| rF2VehicleTelemetry | `mTCActive` | `u8` | 1 if TC currently intervening |
| rF2VehicleTelemetry | `mSpeedLimiterActive` | `u8` | 1 if speed limiter currently active |
| rF2VehicleTelemetry | `mWiperState` | `u8` | wiper state |
| rF2VehicleTelemetry | `mTC` | `u8` | traction control level |
| rF2VehicleTelemetry | `mTCMax` | `u8` | TC max level for this car |
| rF2VehicleTelemetry | `mTCSlip` | `u8` | TC slip threshold |
| rF2VehicleTelemetry | `mTCSlipMax` | `u8` | TC slip max |
| rF2VehicleTelemetry | `mTCCut` | `u8` | TC cut level |
| rF2VehicleTelemetry | `mTCCutMax` | `u8` | TC cut max |
| rF2VehicleTelemetry | `mABS` | `u8` | ABS level |
| rF2VehicleTelemetry | `mABSMax` | `u8` | ABS max level for this car |
| rF2VehicleTelemetry | `mMotorMap` | `u8` | engine/motor map |
| rF2VehicleTelemetry | `mMotorMapMax` | `u8` | motor map max |
| rF2VehicleTelemetry | `mMigration` | `u8` | brake migration step |
| rF2VehicleTelemetry | `mMigrationMax` | `u8` | brake migration max |
| rF2VehicleTelemetry | `mFrontAntiSway` | `u8` | front anti-roll bar level |
| rF2VehicleTelemetry | `mFrontAntiSwayMax` | `u8` | front ARB max |
| rF2VehicleTelemetry | `mRearAntiSway` | `u8` | rear anti-roll bar level |
| rF2VehicleTelemetry | `mRearAntiSwayMax` | `u8` | rear ARB max |
| rF2VehicleTelemetry | `mLiftAndCoastProgress` | `u8` | lift-and-coast progress |
| rF2VehicleTelemetry | `mTrackLimitsSteps` | `u8` | normalized track limits points |
| rF2VehicleTelemetry | `mRegen` | `f32` | regeneration (kW) |
| rF2VehicleTelemetry | `mSoC` | `f32` | state of charge |
| rF2VehicleTelemetry | `mVirtualEnergy` | `f32` | virtual energy fraction |
| rF2VehicleTelemetry | `mTimeGapCarAhead` | `f32` | time gap to car directly ahead (s) |
| rF2VehicleTelemetry | `mTimeGapCarBehind` | `f32` | time gap to car directly behind (s) |
| rF2VehicleTelemetry | `mTimeGapPlaceAhead` | `f32` | time gap to position ahead (s) |
| rF2VehicleTelemetry | `mTimeGapPlaceBehind` | `f32` | time gap to position behind (s) |
| rF2VehicleTelemetry | `mVehicleModel` | `[u8; 30]` | vehicle model name |
| rF2VehicleTelemetry | `mVehicleClassEnum` | `u8` | IP_VehicleClass enum (0=Hypercar, 5=GT3, ...) |
| rF2VehicleTelemetry | `mVehicleChampionshipEnum` | `u8` | IP_VehicleChampionship enum |
| rF2VehicleTelemetry | `mExpansion` | `[u8; 20]` | remaining future-use bytes |
| rF2VehicleTelemetry | `mWheels` | `[rF2Wheel; 4]` |  |
| rF2VehicleScoring | `mID` | `i32` | slot ID |
| rF2VehicleScoring | `mDriverName` | `[u8; 32]` | driver name (UTF-8) |
| rF2VehicleScoring | `mVehicleName` | `[u8; 64]` | vehicle name |
| rF2VehicleScoring | `mTotalLaps` | `i16` | completed laps |
| rF2VehicleScoring | `mSector` | `i8` | 0=S3, 1=S1, 2=S2 (delayed by one sector) |
| rF2VehicleScoring | `mFinishStatus` | `i8` | 0=none,1=finished,2=DNF,3=DQ |
| rF2VehicleScoring | `mLapDist` | `f64` | distance around track (m) |
| rF2VehicleScoring | `mPathLateral` | `f64` | lateral offset from centre path |
| rF2VehicleScoring | `mTrackEdge` | `f64` | track edge w.r.t. centre path |
| rF2VehicleScoring | `mBestSector1` | `f64` | best S1 time (s) |
| rF2VehicleScoring | `mBestSector2` | `f64` | best S2 cumulative time (s) |
| rF2VehicleScoring | `mBestLapTime` | `f64` | best lap time (s) |
| rF2VehicleScoring | `mLastSector1` | `f64` | last S1 time (s) |
| rF2VehicleScoring | `mLastSector2` | `f64` | last S2 cumulative time (s) |
| rF2VehicleScoring | `mLastLapTime` | `f64` | last lap time (s) |
| rF2VehicleScoring | `mCurSector1` | `f64` | current S1 time (s) if valid |
| rF2VehicleScoring | `mCurSector2` | `f64` | current S2 cumulative time (s) if valid |
| rF2VehicleScoring | `mNumPitstops` | `i16` | pitstops made |
| rF2VehicleScoring | `mNumPenalties` | `i16` | outstanding penalties |
| rF2VehicleScoring | `mIsPlayer` | `u8` | 1 if this is the local player |
| rF2VehicleScoring | `mControl` | `i8` | -1=nobody,0=local player,1=local AI,2=remote,3=replay |
| rF2VehicleScoring | `mInPits` | `u8` | 1 if between pit_entry and pit_exit |
| rF2VehicleScoring | `mPlace` | `u8` | 1-based race position |
| rF2VehicleScoring | `mVehicleClass` | `[u8; 32]` | vehicle class name |
| rF2VehicleScoring | `mTimeBehindNext` | `f64` | gap to the car ahead (s) |
| rF2VehicleScoring | `mLapsBehindNext` | `i32` | laps behind car ahead |
| rF2VehicleScoring | `mTimeBehindLeader` | `f64` | gap to leader (s) |
| rF2VehicleScoring | `mLapsBehindLeader` | `i32` | laps behind leader |
| rF2VehicleScoring | `mLapStartET` | `f64` | session time when this lap started |
| rF2VehicleScoring | `mPos` | `rF2Vec3` | world position (m) |
| rF2VehicleScoring | `mLocalVel` | `rF2Vec3` | local velocity (m/s) |
| rF2VehicleScoring | `mLocalAccel` | `rF2Vec3` | local acceleration (m/s²) |
| rF2VehicleScoring | `mOri` | `[rF2Vec3; 3]` | orientation matrix rows |
| rF2VehicleScoring | `mLocalRot` | `rF2Vec3` | angular velocity in local coords (rad/s) |
| rF2VehicleScoring | `mLocalRotAccel` | `rF2Vec3` | angular acceleration in local coords (rad/s²) |
| rF2VehicleScoring | `mHeadlights` | `u8` | headlight status |
| rF2VehicleScoring | `mPitState` | `u8` | 0=none,1=request,2=entering,3=stopped,4=exiting |
| rF2VehicleScoring | `mServerScored` | `u8` | 1 if scored by server |
| rF2VehicleScoring | `mIndividualPhase` | `u8` | game phase for this vehicle |
| rF2VehicleScoring | `mQualification` | `i32` | 1-based qualifying position (-1 if invalid) |
| rF2VehicleScoring | `mTimeIntoLap` | `f64` | estimated time into current lap (s) |
| rF2VehicleScoring | `mEstimatedLapTime` | `f64` | estimated total lap time (s) |
| rF2VehicleScoring | `mPitGroup` | `[u8; 24]` | pit group identifier |
| rF2VehicleScoring | `mFlag` | `u8` | primary flag shown to this vehicle |
| rF2VehicleScoring | `mUnderYellow` | `u8` | 1 if full-course yellow active for this car |
| rF2VehicleScoring | `mCountLapFlag` | `u8` | 0=uncounted, 1=lap only, 2=lap and time |
| rF2VehicleScoring | `mInGarageStall` | `u8` | 1 if within allowable garage area |
| rF2VehicleScoring | `mUpgradePack` | `[u8; 16]` | upgrade pack code |
| rF2VehicleScoring | `mPitLapDist` | `f32` | pit lane distance (m)  ← f32, not f64! |
| rF2VehicleScoring | `mBestLapSector1` | `f32` | S1 time from best lap (s)  ← f32! |
| rF2VehicleScoring | `mBestLapSector2` | `f32` | S2 cumulative from best lap (s)  ← f32! |
| rF2VehicleScoring | `mExpansion` | `[u8; 48]` | reserved |
| rF2ScoringInfo | `mTrackName` | `[u8; 64]` | track name |
| rF2ScoringInfo | `mSession` | `i32` | 0=testday,1-4=practice,5-8=qual,9=warmup,10-13=race |
| rF2ScoringInfo | `mCurrentET` | `f64` | current session time (s) |
| rF2ScoringInfo | `mEndET` | `f64` | session end time (s) |
| rF2ScoringInfo | `mMaxLaps` | `i32` | max laps (999999 if time-based) |
| rF2ScoringInfo | `mLapDist` | `f64` | full lap distance (m) |
| rF2ScoringInfo | `mPointer1` | `[u8; 8]` | padding for pointer (64-bit plugin) |
| rF2ScoringInfo | `mNumVehicles` | `i32` | vehicles currently in session |
| rF2ScoringInfo | `mGamePhase` | `u8` | 0=garage,1=warmup,2=gridwalk,3=formation,... |
| rF2ScoringInfo | `mYellowFlagState` | `i8` | 0=none,1=pending,2=pits closed,3=pit lead lap,4=pits open,5=last lap,6=resume,7=race halt |
| rF2ScoringInfo | `mSectorFlag` | `[i8; 3]` | local yellow in each sector |
| rF2ScoringInfo | `mStartLight` | `u8` | start light frame number |
| rF2ScoringInfo | `mNumRedLights` | `u8` | red lights in start sequence |
| rF2ScoringInfo | `mInRealtime` | `u8` | 1 if in realtime (not at monitor/menu) |
| rF2ScoringInfo | `mPlayerName` | `[u8; 32]` | local player name |
| rF2ScoringInfo | `mPlrFileName` | `[u8; 64]` | player file name |
| rF2ScoringInfo | `mDarkCloud` | `f64` | cloud darkness 0.0–1.0 |
| rF2ScoringInfo | `mRaining` | `f64` | rain severity 0.0–1.0 |
| rF2ScoringInfo | `mAmbientTemp` | `f64` | air temperature (Celsius) |
| rF2ScoringInfo | `mTrackTemp` | `f64` | track temperature (Celsius) |
| rF2ScoringInfo | `mWind` | `rF2Vec3` | wind vector (m/s) |
| rF2ScoringInfo | `mMinPathWetness` | `f64` | minimum wetness on racing line (0–1) |
| rF2ScoringInfo | `mMaxPathWetness` | `f64` | maximum wetness on racing line (0–1) |
| rF2ScoringInfo | `mGameMode` | `u8` | 1=single-player,2=multiplayer,3=competition |
| rF2ScoringInfo | `mIsPasswordProtected` | `u8` | 1 if server is password protected |
| rF2ScoringInfo | `mServerPort` | `u16` | server port |
| rF2ScoringInfo | `mServerPublicIP` | `u32` | server public IP (packed u32) |
| rF2ScoringInfo | `mMaxPlayers` | `i32` | max players in server |
| rF2ScoringInfo | `mServerName` | `[u8; 32]` | server name |
| rF2ScoringInfo | `mStartET` | `f32` | event start time (seconds since midnight)  ← f32! |
| rF2ScoringInfo | `mAvgPathWetness` | `f64` | average wetness on racing line (0–1) |
| rF2ScoringInfo | `mSessionTimeRemaining` | `f32` | seconds left in session |
| rF2ScoringInfo | `mTimeOfDay` | `f32` | seconds since midnight |
| rF2ScoringInfo | `mIsFixedSetup` | `u8` | 1 if the session enforces a fixed setup |
| rF2ScoringInfo | `mTrackGripLevel` | `u8` | 0=green,1=low,2=medium,3=high,4=saturated |
| rF2ScoringInfo | `mCloudCoverage` | `u8` |  |
| rF2ScoringInfo | `mTrackLimitsStepsPerPenalty` | `u8` | steps that add up to one penalty |
| rF2ScoringInfo | `mTrackLimitsStepsPerPoint` | `u8` |  |
| rF2ScoringInfo | `mExpansion` | `[u8; 187]` | reserved (was 200 before the fields above) |
| rF2ScoringInfo | `mPointer2` | `[u8; 8]` | padding for pointer (64-bit plugin) |

## bridge/src/shared_memory/lmu_data.rs

| Record | Field | Rust representation | Mapping note |
|---|---|---|---|
| LmuEvent | `SME_ENTER` | `u32` |  |
| LmuEvent | `SME_EXIT` | `u32` |  |
| LmuEvent | `SME_STARTUP` | `u32` |  |
| LmuEvent | `SME_SHUTDOWN` | `u32` |  |
| LmuEvent | `SME_LOAD` | `u32` |  |
| LmuEvent | `SME_UNLOAD` | `u32` |  |
| LmuEvent | `SME_START_SESSION` | `u32` |  |
| LmuEvent | `SME_END_SESSION` | `u32` |  |
| LmuEvent | `SME_ENTER_REALTIME` | `u32` |  |
| LmuEvent | `SME_EXIT_REALTIME` | `u32` |  |
| LmuEvent | `SME_UPDATE_SCORING` | `u32` |  |
| LmuEvent | `SME_UPDATE_TELEMETRY` | `u32` |  |
| LmuEvent | `SME_INIT_APPLICATION` | `u32` |  |
| LmuEvent | `SME_UNINIT_APPLICATION` | `u32` |  |
| LmuEvent | `SME_SET_ENVIRONMENT` | `u32` |  |
| LmuEvent | `SME_FFB` | `u32` |  |
| LmuApplicationState | `mAppWindow` | `u64` |  |
| LmuApplicationState | `mWidth` | `u32` |  |
| LmuApplicationState | `mHeight` | `u32` |  |
| LmuApplicationState | `mRefreshRate` | `u32` |  |
| LmuApplicationState | `mWindowed` | `u32` |  |
| LmuApplicationState | `mOptionsLocation` | `u8` |  |
| LmuApplicationState | `mOptionsPage` | `[u8; 31]` |  |
| LmuApplicationState | `mExpansion` | `[u8; 204]` |  |
| LmuGeneric | `events` | `LmuEvent` |  |
| LmuGeneric | `gameVersion` | `i32` |  |
| LmuGeneric | `FFBTorque` | `f32` |  |
| LmuGeneric | `appInfo` | `LmuApplicationState` |  |
| LmuPathData | `userData` | `[u8; 260]` |  |
| LmuPathData | `customVariables` | `[u8; 260]` |  |
| LmuPathData | `stewardResults` | `[u8; 260]` |  |
| LmuPathData | `playerProfile` | `[u8; 260]` |  |
| LmuPathData | `pluginsFolder` | `[u8; 260]` |  |
| LmuScoringData | `scoringInfo` | `rF2ScoringInfo` |  |
| LmuScoringData | `scoringStreamSize` | `[u8; 12]` |  |
| LmuScoringData | `vehScoringInfo` | `[rF2VehicleScoring; MAX_MAPPED_VEHICLES]` |  |
| LmuScoringData | `scoringStream` | `[u8; 65536]` |  |
| LmuTelemetryData | `activeVehicles` | `u8` |  |
| LmuTelemetryData | `playerVehicleIdx` | `u8` |  |
| LmuTelemetryData | `playerHasVehicle` | `u8` |  |
| LmuTelemetryData | `telemInfo` | `[rF2VehicleTelemetry; MAX_MAPPED_VEHICLES]` |  |
| LmuObjectOut | `generic` | `LmuGeneric` |  |
| LmuObjectOut | `paths` | `LmuPathData` |  |
| LmuObjectOut | `scoring` | `LmuScoringData` |  |
| LmuObjectOut | `telemetry` | `LmuTelemetryData` |  |

## bridge/src/engineer/legacy_types.rs

| Record | Field | Rust representation | Mapping note |
|---|---|---|---|
| LegacyWheel | `mSuspensionDeflection` | `f64` | meters |
| LegacyWheel | `mRideHeight` | `f64` | meters |
| LegacyWheel | `mSuspForce` | `f64` | pushrod load in Newtons |
| LegacyWheel | `mBrakeTemp` | `f64` | Kelvin. Both the rF2 SDK header and LMU's own |
| LegacyWheel | `mBrakePressure` | `f64` | 0.0-1.0 (future: true kPa) |
| LegacyWheel | `mRotation` | `f64` | radians/sec |
| LegacyWheel | `mLateralPatchVel` | `f64` | m/s lateral velocity at contact patch |
| LegacyWheel | `mLongitudinalPatchVel` | `f64` | m/s longitudinal velocity at contact patch |
| LegacyWheel | `mLateralGroundVel` | `f64` | m/s lateral ground velocity |
| LegacyWheel | `mLongitudinalGroundVel` | `f64` | m/s longitudinal ground velocity |
| LegacyWheel | `mCamber` | `f64` | radians |
| LegacyWheel | `mLateralForce` | `f64` | Newtons |
| LegacyWheel | `mLongitudinalForce` | `f64` | Newtons |
| LegacyWheel | `mTireLoad` | `f64` | Newtons |
| LegacyWheel | `mGripFract` | `f64` | fraction of patch that is sliding (0-1) |
| LegacyWheel | `mPressure` | `f64` | kPa tire air pressure |
| LegacyWheel | `mTemperature` | `[f64; 3]` | Kelvin: left/center/right |
| LegacyWheel | `mWear` | `f64` | 1.0 (new) – 0.0 (destroyed) [LMU convention; inverted vs. rF2 plugin docs] |
| LegacyWheel | `mTerrainName` | `[u8; 16]` | material prefix from TDF |
| LegacyWheel | `mSurfaceType` | `u8` | 0=dry,1=wet,2=grass,3=dirt,4=gravel,5=rumble,6=special |
| LegacyWheel | `mFlat` | `u8` | 1 if flat |
| LegacyWheel | `mDetached` | `u8` | 1 if detached |
| LegacyWheel | `mStaticUndeflectedRadius` | `u8` | tire radius in centimetres |
| LegacyWheel | `mVerticalTireDeflection` | `f64` | deflection from static radius |
| LegacyWheel | `mWheelYLocation` | `f64` | wheel Y relative to vehicle Y |
| LegacyWheel | `mToe` | `f64` | current toe angle (rad) w.r.t. vehicle |
| LegacyWheel | `mTireCarcassTemperature` | `f64` | rough average carcass temperature (Kelvin) |
| LegacyWheel | `mTireInnerLayerTemperature` | `[f64; 3]` | rough average inner-layer temperatures (Kelvin) |
| LegacyWheel | `expansion` | `[u8; 24]` |  |
| LegacyTelemetry | `mID` | `i32` | slot ID (may change post-init) |
| LegacyTelemetry | `mDeltaTime` | `f64` | seconds since last update |
| LegacyTelemetry | `mElapsedTime` | `f64` | game session time (seconds) |
| LegacyTelemetry | `mLapNumber` | `i32` | current lap (0-based) |
| LegacyTelemetry | `mLapStartET` | `f64` | elapsed time when this lap started |
| LegacyTelemetry | `mVehicleName` | `[u8; 64]` | vehicle model name |
| LegacyTelemetry | `mTrackName` | `[u8; 64]` | track name |
| LegacyTelemetry | `mPos` | `rF2Vec3` | world position (metres) |
| LegacyTelemetry | `mLocalVel` | `rF2Vec3` | velocity in local coords (m/s) |
| LegacyTelemetry | `mLocalAccel` | `rF2Vec3` | acceleration in local coords (m/s²) |
| LegacyTelemetry | `mOri` | `[rF2Vec3; 3]` | orientation matrix rows (local→world) |
| LegacyTelemetry | `mLocalRot` | `rF2Vec3` | angular velocity in local coords (rad/s) |
| LegacyTelemetry | `mLocalRotAccel` | `rF2Vec3` | angular acceleration in local coords (rad/s²) |
| LegacyTelemetry | `mGear` | `i32` | -1=reverse, 0=neutral, 1+=forward |
| LegacyTelemetry | `mEngineRPM` | `f64` | current RPM  ← BEFORE mEngineMaxRPM |
| LegacyTelemetry | `mEngineWaterTemp` | `f64` | Celsius |
| LegacyTelemetry | `mEngineOilTemp` | `f64` | Celsius |
| LegacyTelemetry | `mClutchRPM` | `f64` | clutch-side RPM |
| LegacyTelemetry | `mUnfilteredThrottle` | `f64` | 0.0 – 1.0 |
| LegacyTelemetry | `mUnfilteredBrake` | `f64` | 0.0 – 1.0 |
| LegacyTelemetry | `mUnfilteredSteering` | `f64` | -1.0 (left) – 1.0 (right) |
| LegacyTelemetry | `mUnfilteredClutch` | `f64` | 0.0 – 1.0 |
| LegacyTelemetry | `mFilteredThrottle` | `f64` | 0.0 – 1.0 |
| LegacyTelemetry | `mFilteredBrake` | `f64` | 0.0 – 1.0 |
| LegacyTelemetry | `mFilteredSteering` | `f64` | -1.0 – 1.0 |
| LegacyTelemetry | `mFilteredClutch` | `f64` | 0.0 – 1.0 |
| LegacyTelemetry | `mSteeringShaftTorque` | `f64` | Nm (useful for FFB) |
| LegacyTelemetry | `mFront3rdDeflection` | `f64` | front third-spring deflection (m) |
| LegacyTelemetry | `mRear3rdDeflection` | `f64` | rear third-spring deflection (m) |
| LegacyTelemetry | `mFrontWingHeight` | `f64` | front wing height (m) |
| LegacyTelemetry | `mFrontRideHeight` | `f64` | front ride height (m) |
| LegacyTelemetry | `mRearRideHeight` | `f64` | rear ride height (m) |
| LegacyTelemetry | `mDrag` | `f64` | drag force (Newtons) |
| LegacyTelemetry | `mFrontDownforce` | `f64` | front downforce (Newtons) |
| LegacyTelemetry | `mRearDownforce` | `f64` | rear downforce (Newtons) |
| LegacyTelemetry | `mFuel` | `f64` | current fuel level (litres) |
| LegacyTelemetry | `mEngineMaxRPM` | `f64` | redline RPM  ← AFTER mFuel |
| LegacyTelemetry | `mScheduledStops` | `u8` | scheduled pit stops remaining |
| LegacyTelemetry | `mOverheating` | `u8` | 1 if overheating icon shown |
| LegacyTelemetry | `mDetached` | `u8` | 1 if any part is detached |
| LegacyTelemetry | `mHeadlights` | `u8` | 1 if headlights on |
| LegacyTelemetry | `mDentSeverity` | `[u8; 8]` | 0=none,1=dented,2=very dented (8 locations) |
| LegacyTelemetry | `mLastImpactET` | `f64` | session time of last impact |
| LegacyTelemetry | `mLastImpactMagnitude` | `f64` | magnitude of last impact |
| LegacyTelemetry | `mLastImpactPos` | `rF2Vec3` | world position of last impact |
| LegacyTelemetry | `mEngineTorque` | `f64` | engine torque at wheels (Nm) |
| LegacyTelemetry | `mCurrentSector` | `i32` | 0=S1, 1=S2, 2=S3 (sign bit = in pitlane) |
| LegacyTelemetry | `mSpeedLimiter` | `u8` | 1 if pit speed limiter active |
| LegacyTelemetry | `mMaxGears` | `u8` | number of forward gears |
| LegacyTelemetry | `mFrontTireCompoundIndex` | `u8` | compound index (front) |
| LegacyTelemetry | `mRearTireCompoundIndex` | `u8` | compound index (rear) |
| LegacyTelemetry | `mFuelCapacity` | `f64` | tank capacity (litres) |
| LegacyTelemetry | `mFrontFlapActivated` | `u8` | 1 if front flap activated |
| LegacyTelemetry | `mRearFlapActivated` | `u8` | 1 if rear flap activated |
| LegacyTelemetry | `mRearFlapLegalStatus` | `u8` | 0=disallowed,1=detected/pending,2=allowed |
| LegacyTelemetry | `mIgnitionStarter` | `u8` | 0=off,1=ignition,2=ignition+starter |
| LegacyTelemetry | `mFrontTireCompoundName` | `[u8; 18]` | compound name (front) |
| LegacyTelemetry | `mRearTireCompoundName` | `[u8; 18]` | compound name (rear) |
| LegacyTelemetry | `mSpeedLimiterAvailable` | `u8` | 1 if speed limiter available |
| LegacyTelemetry | `mAntiStallActivated` | `u8` | 1 if anti-stall active |
| LegacyTelemetry | `mUnused` | `[u8; 2]` |  |
| LegacyTelemetry | `mVisualSteeringWheelRange` | `f32` | visual lock-to-lock range (degrees) |
| LegacyTelemetry | `mRearBrakeBias` | `f64` | rear brake bias fraction |
| LegacyTelemetry | `mTurboBoostPressure` | `f64` | turbo boost (bar) |
| LegacyTelemetry | `mPhysicsToGraphicsOffset` | `[f32; 3]` | physics→graphics centre offset |
| LegacyTelemetry | `mPhysicalSteeringWheelRange` | `f32` | physical lock-to-lock range (degrees) |
| LegacyTelemetry | `mBatteryChargeFraction` | `f64` |  |
| LegacyTelemetry | `mElectricBoostMotorTorque` | `f64` |  |
| LegacyTelemetry | `mElectricBoostMotorRPM` | `f64` |  |
| LegacyTelemetry | `mElectricBoostMotorTemperature` | `f64` |  |
| LegacyTelemetry | `mElectricBoostWaterTemperature` | `f64` |  |
| LegacyTelemetry | `mElectricBoostMotorState` | `u8` |  |
| LegacyTelemetry | `expansion` | `[u8; 111]` |  |
| LegacyTelemetry | `mWheels` | `[LegacyWheel; 4]` |  |
