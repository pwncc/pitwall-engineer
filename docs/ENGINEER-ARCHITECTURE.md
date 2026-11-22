# Pitwall Engineer architecture

## Processes and trust boundaries

`lmu-agent.exe` runs on each driving PC. It captures the local LMU native mapping by default, acquires the SDK lock briefly while copying, and releases it before any processing, disk or network work. The legacy path has independent packed structures, mapping headers and version witnesses. It never reinterprets legacy buffers as the native ABI.

`lmu-engineer.exe` runs on the engineer PC and owns recording, analysis and the embedded dashboard. HTTP binds to loopback only (default 7437). TLS telemetry ingestion binds to network interfaces (default 7443). Dashboard Host and Origin checks reject unrelated origins and DNS-rebinding-style hostnames. The remote listener accepts no browser dashboard or arbitrary game-control commands.

Pairing creates a random source ID and private random token scoped to one event and car entry. The driver EXE carries an appended configuration footer containing endpoint, identity, token and the receiver certificate. TLS trusts only that paired certificate; there is no certificate-verification bypass. A source can be revoked during an active connection. Each car can receive different co-driver sources over time, while source conflict and spectator gating protect active driver data.

## Capture and transmission

Acquired samples carry source wall time, game time, session identity, local-player authority, driving channels, resources, wheels, electronics, conditions and available scoring. Acquisition defaults to 50 Hz. Scoring is retained at approximately 5 Hz. Batches flush approximately once a second, including lower-rate spectator feeds. The protocol is version 1, length-prefixed zstd-compressed MessagePack over TLS, with a 4 MiB compressed and 16 MiB decompressed frame limit. Batches contain at most 200 samples.

The sampling thread hands one-second batches to a separate disk worker through a bounded 30-batch queue. Brief filesystem flush pauses therefore do not block sampling. A full queue applies backpressure without dropping an acquired batch; the worker drains before capture reports completion. The collector writes durable spool files before transmission. Fresh data is prioritised after reconnect; older batches are marked historical. It removes a batch only after an accepted sequence acknowledgement. Idle heartbeats keep the connection open without making telemetry look fresh. Reconnect uses bounded exponential backoff. Each paired source has an OS file lock to avoid duplicate collectors on the same PC. A 4 GiB spool ceiling stops capture with a local status rather than discarding evidence.

## Storage and reconstruction

The receiver writes compressed append-only frames, flushes them, then commits a SQLite WAL/FULL index. A unique `(source, run, sequence)` key deduplicates retries. Generated hashed chunk names prevent identities becoming paths. A frame written before an interrupted index commit is an ignored tail; the sender's retained batch can be acknowledged only after a committed index record.

Completed lap traces are separately compressed and indexed. Recent traces and a fastest valid reference remain in memory, while historical lap metadata is bounded. Discarded traces release their allocation, and state metadata is copied without cloning sample buffers. CSV export decodes and streams through a bounded channel outside the live ingestion mutex. Playback endpoints are bounded and decimated for the browser; full acquired samples remain in the archive.

Historical arrivals mark an entry for reconciliation. A background worker rebuilds its lap/drive-time history in capture order using a separate database connection. It catches up the short live tail, preserves the last live-receipt time and only replaces the reconstructed state when it is at least as current. Event identities are deterministic during replay so restarts do not duplicate automatic events. Startup reconstruction labels all restored entries stale until a fresh driver batch arrives.

## Analysis contracts

Only complete, valid, clean local-driver laps can train consumption. Resource increases, gaps, pit/yellow laps, partial starts, incompatible sessions and driver-change fragments are excluded. Median pace/fuel/VE windows are bounded. Source identity is separate from driver name and stable car entry. A spectator is never authoritative for fuel, energy or wheel state.

Scenario assumptions are explicit and editable. Per-car service overrides sit over event defaults. Confirmed tyre counts constrain service feasibility; driving schedules and configured limits generate reminders. Scenario times compare a fixed forecast distance, so service cost cannot improve the estimate by removing race distance. Native-imported references are descriptive and excluded from live learning/advice.

The UI polls state at 2 Hz and replay/lap information at 5-second intervals. Workspace layouts and per-panel car choices are saved in browser local storage; event rules, credentials, notes and recordings live in the receiver data directory. Review delay affects the workbench rather than the live strategy model.

## Supplement and native files

The optional REST worker has a fixed local allowlist, short timeouts, bounded response reads and progressive backoff. It is active only for authoritative driving capture when enabled. Cached data expires after 15 seconds. Missing forecast timestamps and missing channels remain explicit.

Native DuckDB import uses the official C ABI with a local runtime. Database access is read-only, memory-limited, limited to two worker threads, with external access and automatic extension installation/loading disabled. Only application-generated queries execute. Query row/cell/file limits bound import work, which runs outside the ingestion mutex. Units must be confirmed by the operator. The native importer currently omits spatial/wheel channels; this is distinct from the full spatial/wheel data in the live collector and its own archives.

## Operational boundaries

No Steam/RaceControl login or backend results/ratings scraping. No remote game-control writes, automatic driver messaging, audio strategy calls or pit-menu changes. The engineer reviews suggestions and acts in LMU/team communications. The release has no cloud relay or automatic router configuration; reachability must be established on the team's actual network. A local eight-car test cannot establish external routing or a real driver handover.

## Receiver extension and update boundary

The collector wire protocol remains version 1 while receiver product versions can change independently. Calculations, API responses and dashboard features operate on the existing samples/standings. The receiver can serve replaceable public UI files from its adjacent `ui` directory; canonical path checks keep requests inside that directory, and embedded assets remain the fallback. `scripts/dev-engineer.ps1` provides a loopback-only hot-reload dashboard with a restricted local API proxy. `scripts/update-engineer-receiver.ps1` preserves identity and stock collector binaries, retires mapped receiver EXEs safely on Windows, and restores the old receiver on startup failure. See `ENGINEER-DEVELOPMENT.md`.

Shared packages add optional handshake enrolment metadata without changing sample protocol 1. First-launch PC identities are persisted locally under a short OS file lock, so duplicate launches reuse the same identity and existing collector singleton. A valid shared credential can enrol up to 16 PC identities (within the receiver's 128-credential ceiling). Enrolment is durable and idempotent; known revoked identities cannot re-authenticate. Whole-package revocation cascades to enrolled credentials. Legacy handshakes omit these fields and continue to authenticate normally.
