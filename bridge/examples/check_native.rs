use lmu_pitwall::shared_memory::reader::SharedMemoryReader;
fn main() {
    let mut reader = SharedMemoryReader::new();
    if !reader.open() {
        println!("No mapping");
        return;
    }
    if let Some(frame) = reader.read_frame() {
        let info = frame.scoring.mScoringInfo;
        let (global, sectors, phase) = (info.mYellowFlagState, info.mSectorFlag, info.mGamePhase);
        println!("global={global} sectors={sectors:?} phase={phase}");
        if let Some(vehicle) = frame.scoring.player() {
            let (under, count, control, lap, distance) = (
                vehicle.mUnderYellow,
                vehicle.mCountLapFlag,
                vehicle.mControl,
                vehicle.mTotalLaps,
                vehicle.mLapDist,
            );
            println!("under={under} count={count} control={control} lap={lap} distance={distance}");
        }
        if let Some(telemetry) = frame.telemetry.player() {
            let (fuel, ve, invalid, elapsed, throttle, gear) = (
                telemetry.mFuel,
                telemetry.mVirtualEnergy,
                telemetry.mLapInvalidated,
                telemetry.mElapsedTime,
                telemetry.mFilteredThrottle,
                telemetry.mGear,
            );
            println!("fuel={fuel} ve={ve} invalid={invalid} elapsed={elapsed} throttle={throttle} gear={gear}");
        }
    }
}
