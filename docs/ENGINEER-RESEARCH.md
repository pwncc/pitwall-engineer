# LMU engineering research and capability assessment

Research date: 21 November 2026. Installed game: LMU v1.4200. Local observations, source contracts and vendor claims are distinguished below. This report does not claim an exhaustive public HTTP API: Studio 397 explicitly says LMU has no supported public API.

## Data interfaces and what they can establish

Studio 397's [November 2026 community update](https://lemansultimate.com/community-update-november-2026/) warns against services using Steam/game credentials to collect ratings and result histories from RaceControl. The implemented collector uses local shared memory and, optionally, a small allowlist of local reads. It performs no account impersonation or RaceControl requests. A local interface being readable does not make it a supported public API.

| Interface | Useful capabilities | Important limits | Engineering decision |
|---|---|---|---|
| Native `LMU_Data` | Driver inputs, motion, fuel, native VE/electronics, wheels, damage; session/scoring and opponents | Per-car channels differ; opponent detailed values can be placeholders; requires a local game instance and plugins enabled | Primary live collector on every driving PC |
| SDK lock objects | Coordinate a coherent snapshot with the writer | Keep ownership extremely short; never hold it for networking or disk writes | Copy under a short lock; analyse afterward |
| rF2 SMMP 3.7 | Legacy telemetry/scoring buffers with version witnesses | Different headers/offsets and channel semantics; native VE/electronics are not supplied by the basic legacy layout | Explicit, separate reduced-capability adapter |
| Local REST service, port 6397 | UI/weather, wearables, selected pit-service estimate | Undocumented, state-sensitive and lower-rate; cannot replace pedal/line acquisition | Optional read-only supplement with timeouts, cache and backoff |
| Native `.duckdb` files | Rich recorded channels at configured frequencies and sparse events | Post-recording workflow; schemas, units and timing need inspection; not a remote live feed | Read-only descriptive reference import |
| Result XML | Classification, laps, sectors and result context | Session summary, not high-rate pedal or line telemetry | Useful independent race audit; not the live transport |
| Video/stream | Driver feedback and visible track context | Delay, compression and missing channels; does not reveal accurate fuel, tyres or inputs | Adjustable workbench delay; telemetry still comes from driver PCs |

The game ships the native headers in `Support/SharedMemoryInterface`. The installed C++ compiler verified telemetry 1888 bytes, wheel 260, scoring 548 and vehicle scoring 584. Native container offsets match the fork; the outer object includes four tail alignment bytes for a total of 324824. Native capacity is 104 slots; legacy buffers use 128. These numbers are verified local ABI observations, not a promise about future game builds. The generated **LMU-CHANNEL-INVENTORY.md** lists the mapped field contract.

The underlying open-source Pitwall reader documents native telemetry at 100 Hz and scoring at 5 Hz. The new agent samples at 50 Hz by default, supports 5–100 Hz, records full acquired samples and retains reduced traces for live analysis. Read consistency, player identity, units and freshness matter more than selecting a nominally high rate. [Pitwall source](https://github.com/Swizzjack/lmu-pitwall), [native mapping reference](https://github.com/TinyPedal/pyLMUSharedMemory).

The [rF2 Shared Memory Map Plugin](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin) exposes several buffer families, including telemetry, scoring, extended state and control-related facilities. This implementation reads only the supported telemetry/scoring/version portions for the legacy path. It does not use DMA, hardware controls, weather controls or pit-control writes.

## Verified local capabilities

A real Monza online practice session was joined without registering a team or purchasing content. In the garage, the new agent produced a scoring-only feed and withheld resource details. In the driving seat, it reported approximately 120 L fuel and 100% VE, with tyre pressures around 117 kPa and engine temperatures matching the visible car displays. The seat transition establishes correct player gating and plausible units; it is not a completed driving-lap or remote handover test.

The implementation distinguishes LMU's sector encoding: `1` means yellow and `11` clear. It checks the local-player flag, control state and matching telemetry ID rather than selecting slot zero. Temperature conversion follows observed Kelvin-to-Celsius behaviour; pressures are shown in kPa. Resource and temperature fields use unavailable values where a valid reading cannot be established.

The optional REST allowlist was checked in the active session:

| Local read | Observed content | Product use |
|---|---|---|
| `/rest/sessions/weather` | Session weather configuration values | Cached supplementary context |
| `/rest/garage/UIScreen/RepairAndRefuel` | Current weather, fuel/VE capacities, wearables, forecast nodes, team info, pit menu and service-time parameters | Forecast nodes and brake thickness; separate from high-rate input data |
| `/rest/strategy/pitstop-estimate` | Fuel, VE, tyres, driver, damage/other service and total estimate for the currently selected menu | Display selected service cost; transit remains additional |

The observed repair/refuel object included brake thickness in metres, tyre remaining fractions and five forecast nodes. Node timestamps were null, so the dashboard displays game order rather than invented times. A car-specific example reported fuel fill rate 4 L/s, VE fill rate approximately 0.025 fraction/s, four-tyre service 12 s and driver service 25 s. These were Vanwall values from that session; they are not assigned as universal GT3 or event rules.

The original Pitwall also uses local strategy-usage data. This build derives consumption from clean driver telemetry laps instead of polling that endpoint. No endpoint sweep is required in the collector. A prior broad exploratory probe coincided with a game crash; causation was not established. The later visible crash reporter was verified to be that earlier process, not a new crash during the stationary collector check. Supplements therefore remain an opt-in feature, isolated from the core shared-memory path.

## Native recording workflow

Studio 397 documents native DuckDB telemetry recording, automatic or key-bound activation, and configurable per-channel frequencies in `UserData/Telemetry/config.json`. It also notes that conversion to other formats is not directly supported. [Official telemetry recording guide](https://guide.lemansultimate.com/hc/en-gb/articles/14524956311695-Telemetry-Recording).

The importer uses the official DuckDB C ABI and read-only access. It lists recorded metadata/channel descriptors and offers lap-time events wholly contained in the recorded GPS-time range. Speed and input units require confirmation. Untimestamped channels are aligned over the recording extent and remain descriptive references, excluded from automatic consumption and technique evidence. Position/wheel-channel import is not part of this initial native importer. [DuckDB C query documentation](https://duckdb.org/docs/current/clients/c/query).

## Existing software examined

These are useful comparison points and alternatives, not compatibility claims for untested releases. Current vendor/source pages were checked; no paid subscriptions or installations were purchased.

| Tool/source | Relevant strengths | What it contributes to this workstation |
|---|---|---|
| [Swizzjack LMU-Pitwall](https://github.com/Swizzjack/lmu-pitwall) | Rust native reader, embedded React dashboard and configurable widgets | MIT source foundation; separate engineer receiver/agent workflow |
| [LMU Telemetry Lab](https://github.com/rabbit20031225/LMU-Telemetry-Lab) | Native recording analysis, distance/time charts, spatial comparison and cross-session references | Native schema comparison point; motivates separating live operations from lap analysis |
| [MyLMU](https://mylmu.app/docs/telemetry/getting-started/recording-in-lmu) | Native DuckDB recording/import and a broader analysis workflow | A post-session alternative and reference-sharing workflow |
| [LMU Engineer](https://lmu-engineer.com/) | LMU-specific engineering/coaching product | Distinct existing product; this fork uses the name Pitwall Engineer |
| [LMU.tools fuel calculator](https://www.lmu.tools/fuel-calculator) | Fuel and virtual-energy planning | Independent manual sanity check for race resources |
| [Setup Hub FAQ](https://lmusetups.whackablemole.com/faq) | LMU setup and pit calculator context | Car/class capability differences should be explicit |
| [Parc Ferme telemetry](https://www.parcferme.cc/telemetry) | Local recording, session sharing and review panels | Missing channels should stay unavailable; projections need evidence |
| [trophi.ai LMU supported content](https://trophiai.atlassian.net/wiki/spaces/TS/pages/817266689/LMU%2BSupported%2BContent) | Driver-coaching support catalogued by car and track | Coaching coverage must be checked for the actual combination |

A broader search found several similarly named Pitwall/team products. Their names alone do not establish that they are the same project as the requested MIT repository, so architecture and source licensing were taken from the repository actually cloned.

## Professional engineering practice and automation

[MoTeC i2](https://www.motec.com.au/products/I2) provides configurable analysis workspaces and mathematical/reporting tools developed with race-team input. The useful pattern here is a stable workspace for each job: race operations, car health, strategy and driver performance. Software should preserve the engineer's ability to inspect the supporting channels rather than replace them with unexplained verdicts.

[AiM Race Studio 3 Analysis](https://aim-sportline.com/docs/racestudio3/manual/html/analysis.html) supports lap/session references, distance-based comparison, splits and channel statistics. This motivates linked distance axes, explicit reference selection and reading braking, minimum speed and exit together. A later brake point by itself does not establish an improvement.

[ATLAS](https://docs.atlas.mclarenapplied.com/introduction/) describes a telemetry ecosystem spanning live acquisition, distribution, storage and analysis. Its professional pattern maps directly onto a small collector per driver, a central engineer receiver, durable history and calculated channels. This project does not claim the sensor coverage, commercial tooling or validation depth of ATLAS.

The following division is an engineering design derived from these patterns and the verified LMU data, rather than a vendor promise:

| Job | Useful direct information | Appropriate automation | Human judgement retained |
|---|---|---|---|
| Run the car | Fuel/VE, temperatures, pressures, wear, damage, penalties | Threshold alarms, rate/range calculations, stale-feed warnings | Driver feedback, pit action and failure diagnosis |
| Manage the race | Timing, class rivals, pit state, remaining duration | Resource-limited stop windows, fill estimates, simple rejoin/catch projections | Traffic, cautions, rival intent and strategic risk |
| Manage endurance constraints | Current driver, observed time, schedule, tyre inventory | Handover reminders, configured limits, inventory feasibility | Correct event rules and reconciliation of unrecorded time |
| Improve corners | Distance, speed, brake/throttle/steering, position | Section losses and repeated patterns against compatible references | Causal interpretation and coaching instruction |
| Maintain evidence | Raw frames, source identity, timestamps, excluded-lap reasons | Recording, deduplication, replay, backfill and export | Corrections and approval of assumptions |

A causal driving recommendation needs more than the fastest lap. Match car, track, compound, fuel range and weather; exclude traffic/pit/yellow/partial laps; examine several repeats; and expose the confidence/evidence. A strategy estimate similarly needs conservative reserves and a visible distinction between measured service rates and assumed saving/tyre effects. The implemented suggestions are deterministic calculations with displayed evidence, not an opaque language-model decision loop.

## Event observations relevant to the six-hour plan

The selected special-event UI showed Monza 5.793 km, 5-minute practice, 15-minute qualifying, 360-minute race, 38-car split, open setup, fair-share rule, 20 tyres and no tyre warmers. The game's timezone setting was observed as UTC. These observations describe the event inspected, not the user's confirmed registration, chosen class or start slot. The engineer still needs the team's actual entry, stint plan and applicable drive-time values. [Official special-event calendar](https://lemansultimate.com/special-events-calendar-q3-4-2026/).
