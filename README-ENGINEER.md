# Pitwall Engineer for Le Mans Ultimate

A multi-car engineering workstation built on the MIT-licensed [Swizzjack/lmu-pitwall](https://github.com/Swizzjack/lmu-pitwall) source. This is a separate project from other products using the name LMU Engineer or LMU Pitwall.

## Start the workstation

Extract the complete release folder and double-click **lmu-engineer.exe**. It opens **http://127.0.0.1:7437**. Keep `lmu-agent.exe` beside the receiver so it can export driver packages. `duckdb.dll` is needed only for native recording import. That optional official DLL also requires the [Microsoft Visual C++ v14 x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist); live acquisition and both EXEs use a static C runtime. Drivers receive a separately paired EXE and do not need that DLL, Node, Python or Rust.

The receiver stores its data in `%LOCALAPPDATA%\LMUEngineer`. The collector uses `%LOCALAPPDATA%\LMUEngineerAgent\<source-id>`. Closing the dashboard tab leaves the receiver recording. The driver agent has a tray menu with status and Stop. The receiver tray menu opens the dashboard or stops the receiver after the event. Reopening the EXE restores its archive and pairs; use the same data directory throughout the event.

## Connect the team

1. Open **Setup & event rules → Drivers**.
2. Enter the engineer PC's reachable **hostname:7443**. Use its LAN address for a local test, a private network address for VPN peers, or a public address with TCP port forwarding for Internet peers. `127.0.0.1` works only when collector and receiver run on the same PC.
3. Give every car a stable entry identifier, such as `Car 27 GT3`. Keep **One sender EXE for all co-drivers** enabled. Export one shared package for the car; each driving PC automatically creates and saves an independent identity. Different cars use different entry identifiers.
4. Send the same shared ZIP/EXE to all co-drivers of that car. They extract it and launch its EXE before driving. Each package contains a private token and trust for this receiver's certificate; use Revoke on the shared package to disconnect its enrolled PCs and prevent new enrolments. Individual per-PC packages remain available by clearing the shared-package option.
5. Enable **LMU → Settings → Gameplay → Enable Plugins** on the driving PC. Native `LMU_Data` is the preferred source; the extra rF2 plugin is not required for native capture.
6. Confirm that the correct driver's name, car, fuel and virtual energy appear, and compare them with the in-game displays before the race.

For public hosting, forward **TCP 7443** on your router to the engineer PC's LAN address and permit inbound access to the receiver in Windows Firewall. Router configuration and an actual external-PC connection are deployment checks; loopback tests do not prove them. If your ISP uses CGNAT, use a reachable private network or a host with inbound connectivity. Keep the dashboard port **7437** local; only telemetry ingestion needs remote access. This app does not alter your firewall or router automatically.

The optional REST supplement reads three local game endpoints for forecast, brake thickness and the selected pit estimate. Wind is displayed in km/h and brake thickness in mm. Supplementary values expire after 15 seconds or when the driving feed becomes stale. The five forecast nodes have no supplied timestamps. Enable it in the pairing dialog only when wanted. It is off by default. The legacy adapter is also an explicit pairing choice and supports rF2 SMMP 3.7 buffers with reduced channels; it does not read DMA or control buffers.

## During the race

**Race Overview** combines fleet health, prioritised advice, classification, map, resources and notes. **Car Engineering** covers fuel, energy, tyres, brakes and vehicle systems. **Strategy** contains resource and service scenarios, class-rival projections and driver plans. **Performance** compares recorded laps and references by distance.

Unlock the layout to drag or resize panels. Each workspace saves its own layout in this browser. Most car-specific panels can follow the focus car or stay assigned to one car. Add panels or restore workspace defaults from **Add panel**. Fleet cards select the focus car. Advice can be acknowledged for the current lap and severity; acknowledgement does not modify the car or mute a later recurrence.

A fresh driver's feed is required for live strategy. Missing or spectator-only resources remain unavailable. A connected collector is visible before the game sends telemetry. A stale panel shows the last recorded values with a warning, and live fill recommendations/scenarios are suppressed. A spectator PC never substitutes its placeholder fuel or tyre values for a driving PC's detailed telemetry. If you watch a stream, keep collecting from the actual driving PCs. The review delay and playback slider affect the telemetry workbench, while live strategy continues to use current telemetry.

## Prepare strategy assumptions

The defaults are an unconfirmed Monza six-hour rehearsal. Confirm the actual event, duration, extra-lap rule, reserves, transit loss, service rates and concurrency in **Event & strategy**. The observed event UI included a fair-share rule and a tyre allocation; the software does not invent fair-share thresholds. Enter the applicable drive-time constraints from your event briefing.

Use **Car service & tyres** for each car's own fuel/energy rates and service/transit times. A blank service value uses the event default. The tyre ledger is manual: enter the confirmed number of tyres used, including pre-existing usage, and update it after fresh tyres are fitted. The strategy model rejects a full-set-at-every-stop scenario when it exceeds this confirmed inventory. An unknown ledger is not an assertion that fresh tyres are available.

Sections use distance in metres. With no manual sections for the recorded track, braking zones are detected from the reference lap. You can enter measured section boundaries and a matching track name to override detection. Driver schedules use stable entry ID, exact driver name and session start/end seconds. The editor validates overlapping plans. Keep the PCs' system clocks synchronized for cross-PC archive ordering. A large clock/delivery discrepancy is flagged. Observed drive time and the initial current-driver stint start when recording starts. A same-driver fuel stop does not reset the driver-stint timer. Joining mid-stint leaves the initial stint duration incomplete; check the earlier start against the team’s records. In Sections & schedules, `drive_time_adjustments` maps entry IDs to driver names and signed seconds; add unrecorded driving or subtract a known error. Corrections apply to displayed totals and limit warnings without changing raw history. They remain saved until edited; clear obsolete corrections when beginning a new race session. Reconcile these totals before relying on minimum or maximum drive-time warnings.

Fuel and VE consumption use the median of up to fifteen eligible laps. At least three complete clean driver laps are required for scenario forecasts. Partial laps, data gaps, pits, invalid laps, yellows, resource replenishment and driver-change fragments are excluded. Imported references do not contribute to consumption learning. Fuel range and VE range are computed separately; the lower available range limits the car. Pit-by estimates apply lap and absolute-resource reserves.

Pit entry can be configured by distance from start/finish; blank uses an explicit start/finish approximation. “Pit now” means the next reachable pit entry, and “Extend one lap” skips it for one further lap. A scenario is rejected if resources cannot reach the required pit opportunity while preserving configured reserves.

Scenarios compare the same nominal forecast distance: pit now, extend one lap, an assumed saving mode, tyre service, driver handover and repair/service. Leader pace and position help estimate the timed-race finish. Future cautions, pace changes and class traffic can change the final lap count. Saving uses an explicitly displayed 3% consumption reduction with a 1% pace cost; tyre and repair pace benefits are unknown and are not invented. Service costs require measurement. Rejoin position uses transit loss only and is labelled accordingly; rival catch estimates use the last reported rival lap and constant pace.

## Recordings and comparisons

The receiver records compressed, indexed telemetry continuously. Choose a completed lap and a reference in Performance. Speed, brake, throttle, steering and time delta use distance as the shared axis. A reference can also overlay the live partial lap. Stream-delay changes and timeline scrubbing return the workbench to timeline mode; they leave live strategy at the present time. Recorded-lap selectors identify the track; the workbench also identifies the recorded car and driver. Corner tables show section loss, brake/throttle application changes, minimum speed and exit speed. Automated corner advice requires a fresh driving feed and three repeated eligible losses from the current driver's current stint against a compatible reference; the user should review approach, apex and exit together.

**Export lap** produces this app's JSON reference format; **Import lap** reads it back. Imported laps are reference-only: they stay in the recording library and Performance selectors, and do not alter live lap evidence or automatic advice. **Export** streams 118 scalar columns without holding up live ingestion. Rows identify the event, entry, source, run, track and car, and include validity/pit/yellow flags, driving inputs, resources, systems, damage, weather and all four wheels' recorded temperatures, pressures, wear, brake temperatures and suspension values. Unavailable optional readings and unsupported electronics stay blank; use the authoritative flag to identify driving-seat channels. The original first 13 columns are retained; context and health channels are appended. Fuel is litres; VE, SOC, tyre remaining, pedal inputs and steering use their recorded normalised values; wheel temperatures are °C, pressure kPa, position/suspension metres and wheel rotation rad/s. Classification arrays and supplementary API objects remain in the raw archive. All raw recorded batches remain in the data directory.

**Import native .duckdb** reads a completed LMU recording on the engineer PC. Inspect the file, select a completely covered lap and confirm the recorded speed/input units. Import uses the official DuckDB runtime in read-only mode, with external access and extension loading disabled. Timestamp-less continuous channels are aligned over the recording extent. Keep any companion `.duckdb.wal` beside the recording when copying it; that file may contain recent telemetry. Native references are therefore descriptive, excluded from automated advice/consumption learning, and carry a warning. This first importer provides distance, speed, driving inputs and available fuel/VE; it does not yet import spatial racing-line coordinates or wheel channels. Live and app-recorded laps do include spatial traces. Unknown native schemas are rejected rather than guessed.

## Outages and recovery

A separate disk worker writes one-second batches through a bounded 30-batch memory queue, so brief filesystem flush pauses can be absorbed while sampling continues. Queued memory is flushed before capture finishes; abrupt process or power loss can lose that in-memory tail. The driver records these batches locally, transmits encrypted compressed frames, and deletes a queued batch only after durable receiver acknowledgement. Reconnect prioritises fresh telemetry, then sends older batches. The receiver deduplicates retries and reconciles delayed history into laps and drive time without replacing fresh values. An interrupted archive index commit leaves an ignored tail; the collector retains the unacknowledged batch for retry.

The offline spool is capped at **4 GiB per paired source**. When that limit is reached capture stops with a local status/error rather than deleting old telemetry. Reconnect and preserve the spool before restarting. Keep free disk space for the race. Receiver archives are not automatically deleted. Preserve the entire receiver data directory, including its certificate and credentials, when moving or backing up the workstation; replacing that identity requires re-pairing collectors.

## Build and development

Use Windows x64, Node 24, Python 3 for the notice-collection helper, Rust 1.96 and Visual Studio C++ build tools/Windows SDK. From the repository run `./scripts/build-engineer.ps1`. It installs locked frontend dependencies, builds the dashboard first, checks the engineer UI and backend tests, builds the release with a static C runtime, collects notices and creates a fresh versioned folder and ZIP under `dist`. The downloaded DuckDB 1.4.0 DLL is verified against a pinned SHA-256.

Useful receiver options: `--data <directory>`, `--http-port 7437`, `--ingest-port 7443`, `--no-browser`. Agent developer options: `--config <json>`, `--hz 50`, `--demo`, `--seconds <simulated duration>`, `--speed <acceleration>`. Demo data is marked synthetic; do not mix it into the real event's data directory.

The original Pitwall application remains in the source tree. Its `/legacy` UI needs the original bridge services and is not the engineering receiver's race workflow. Existing upstream lint findings are distinguished from checks of the new engineer code in the validation report.

See **ENGINEER-CALCULATIONS.md**, **ENGINEER-ARCHITECTURE.md**, **ENGINEER-RESEARCH.md**, **LMU-CHANNEL-INVENTORY.md** and **ENGINEER-VALIDATION.md** in the release folder for implementation, source evidence and test limits.

## Change receiver features during a race

Driver packages and receiver features are independent. UI updates can be applied without restarting telemetry; calculation/API updates use a receiver restart while existing drivers buffer and reconnect. See [ENGINEER-DEVELOPMENT.md](docs/ENGINEER-DEVELOPMENT.md) for live editing, UI deployment, receiver updates and the existing timing data available for relative tables. Packaged copies include that document beside this README.
