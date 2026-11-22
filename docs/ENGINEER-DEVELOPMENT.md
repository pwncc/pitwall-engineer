# Receiver-only feature updates

The receiver and driver agents are separate programs. Keep the existing driver EXEs running while changing the engineer's dashboard, calculations, alerts or reports. The collector protocol remains **version 1**. Receiver product versions are tracked separately in `ENGINEER-VERSION`.

## Which changes need a driver update?

| Change | Where to edit | Driver update |
| --- | --- | --- |
| Panels, tables, plots, layout and browser calculations | `dashboard/src/engineer/` | No |
| Strategy, consumption, comparisons, alerts | `bridge/src/engineer/analysis.rs` | No; rebuild/restart receiver |
| Receiver API and exports | `bridge/src/engineer/server.rs`, `csv_export.rs` | No; rebuild/restart receiver |
| New game channels not already transmitted | `capture.rs`, `legacy.rs`, collector/sample schema | Required on the affected driving PCs |

For example, current agents already send the full field's driver/vehicle/class, positions, lap counts, lap distance, leader gaps, laps behind, best/last laps, pit status and positions on the map. These standings are sent at about 5 Hz inside the 50 Hz telemetry stream. The receiver retains the most recent standings between scoring updates. A relative-times table can be added on the receiving side using this existing data. Unknown game gaps must remain unknown; use one coherent standings snapshot, distinguish lap differences, and avoid treating missing/zero non-leader gaps as measured zero seconds.

Do not bump `model::PROTOCOL` or make newly added sample fields mandatory for a receiver-only feature. Add optional/defaulted inputs when extending the schema, keep accepting existing named MessagePack fields, and verify with an older agent package. Changing a driver's game-memory reader or collecting an untransmitted sensor requires a driver update.

## Edit the dashboard live

Start the usual receiver first. In a source checkout with Node installed:

```powershell
./scripts/dev-engineer.ps1
```

Open **http://127.0.0.1:5173**. Saving dashboard source changes updates that page through Vite/React hot reload. Its `/api/` requests go to the running receiver on port 7437, so the same real telemetry, rules and pairings are available. The receiver and driver processes keep running. This development URL has its own browser layout storage; the usual 7437 dashboard retains its existing layout.

Custom local ports:

```powershell
./scripts/dev-engineer.ps1 -ReceiverPort 17437 -Port 5183
```

The development server binds to loopback. Its proxy rejects foreign browser origins and preserves the receiver's existing Host/Origin restrictions. Stop the development process with Ctrl+C when finished.

## Apply UI changes to the usual dashboard

Receiver **0.1.1 and later** reads public UI files from `ui/` beside `lmu-engineer.exe` on each request. Embedded assets remain the fallback. To build and apply dashboard changes:

```powershell
./scripts/update-engineer-ui.ps1
```

Or target a different receiver installation:

```powershell
./scripts/update-engineer-ui.ps1 -ReceiverDirectory 'D:/Pitwall Engineer'
```

Refresh **http://127.0.0.1:7437** afterwards. The receiver and drivers are not restarted, and the browser's existing workspace stays at the same origin. Compilation must succeed before copying starts. New assets are copied first, the HTML index is replaced atomically last, and old hashed assets remain available to open tabs. Local UI modifications naturally differ from the original release checksums.

## Apply receiver calculation/API changes

Build a complete replacement bundle:

```powershell
./scripts/build-engineer.ps1
```

Then apply the resulting versioned folder:

```powershell
./scripts/update-engineer-receiver.ps1 -BundleDirectory './dist/Pitwall-Engineer-0.1.1-YYYYMMDD-HHMMSS'
```

This backs up the installed receiver, replaces **only its EXE and UI**, and restarts it with the existing data directory and ports. It checks that the event, certificate, collector protocol, driver pairings and installed stock collector EXE remain unchanged. It restores the previous receiver if startup or those checks fail. Old binaries and the old index are retained under `.receiver-backups/` beside the installed receiver.

Drivers keep their current EXEs running. During the receiver restart they spool locally, reconnect to the same certificate and send their backlog. Live data is prioritised, delayed history is reconciled and sequence keys prevent duplicates. The dashboard briefly loses its live connection during restart; a large archive can take about a minute to reconstruct.

For a custom installation use `-ReceiverDirectory`, `-DataDirectory`, `-HttpPort` and `-IngestPort`. Always select the existing receiver profile: replacing its certificate requires pairing drivers again. `-NoTray` is available for isolated tests.

## Verification

`scripts/verify-receiver-upgrade.py --old-bundle <0.1.0 folder> --new-bundle <new folder>` exercises an older, unchanged packaged driver through a failed receiver update/rollback, a successful update, disk UI replacement, browser hot reload and queue draining. It uses an isolated profile and local ports. Its browser checks also verify proxied settings writes and rejection of foreign origins. The test never substitutes a new EXE into the driver's package.

Vite's [server proxy and development options](https://v7.vite.dev/config/server-options) document the local development path. The normal release runs without Node, Rust or a development server.

## One sender file for a whole crew

Receiver 0.1.2 and sender 0.1.1 support a shared package. Its first launch creates a random PC identity under `%LOCALAPPDATA%/LMUEngineerAgent/identities/`; later launches reuse it. The receiver enrols that identity against the shared car credential over the pinned TLS connection. The same EXE can therefore be copied to every co-driver without editing configuration. Revoking the shared package revokes its enrolled PCs too. Keep this paired file private; the unpaired stock sender in the public release contains no team credential. Existing individual packages and older 0.1.0 senders remain supported. This initial shared-package capability needs the new sender; subsequent receiver-only features do not.
