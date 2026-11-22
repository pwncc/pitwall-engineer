# Pitwall Engineer for Le Mans Ultimate

A multi-car workstation for race engineers. Driver PCs run a small paired Windows EXE; telemetry streams to the engineer's receiver for live timing, resources, tyre/brake monitoring, strategy, lap comparisons and corner analysis.

[Download the Windows release](https://github.com/pwncc/pitwall-engineer/releases/latest) · [User guide](README-ENGINEER.md) · [Receiver development](docs/ENGINEER-DEVELOPMENT.md) · [Validation](docs/ENGINEER-VALIDATION.md)

## Run

Extract the complete release ZIP and launch **lmu-engineer.exe**. The dashboard opens at **http://127.0.0.1:7437**. Export paired driver packages from **Setup & event rules → Drivers**. All co-drivers of one car can run the same shared paired EXE; each PC automatically saves its own identity with LMU plugins enabled. Telemetry uses TCP 7443; the dashboard stays local.

Workspaces can be rearranged and saved without a connected car. The receiver records locally, and collectors spool through outages and reconnect automatically. A stream alone does not supply detailed driving telemetry; run the collector on the driving PC.

## Change engineer features without updating drivers

The receiver owns the dashboard, strategy, alerts, comparisons and exports. Existing collectors already send the full field's standings and available gap/position data, so features such as relative-time tables can be added to the receiver.

- `./scripts/dev-engineer.ps1` opens a development dashboard at port 5173 with hot reload, connected to the existing receiver API.
- `./scripts/update-engineer-ui.ps1` builds and applies UI changes; refresh port 7437 without restarting collection.
- `./scripts/update-engineer-receiver.ps1 -BundleDirectory <built-folder>` upgrades the receiver while retaining the profile, certificate and pairings. Drivers buffer and reconnect using their unchanged EXEs.

See [receiver-only development](docs/ENGINEER-DEVELOPMENT.md) for commands, compatibility requirements and the boundary between receiver features and new game channels.

## Build

Windows x64; Node 24, Python 3, Rust 1.96 and Visual Studio C++ build tools/Windows SDK:

```powershell
./scripts/build-engineer.ps1
```

The script checks the engineer UI and backend tests, builds the two portable EXEs, collects dependency notices and produces a versioned folder/ZIP with SHA-256 manifests. Runtime drivers do not need development tools. DuckDB import uses a separately packaged official DLL and the Microsoft x64 VC runtime; live collection does not require it.

## Data and protocol

- Receiver profile: `%LOCALAPPDATA%/LMUEngineer`.
- Collector spool: `%LOCALAPPDATA%/LMUEngineerAgent/<source-id>`.
- Collector wire protocol: version 1, independent of receiver product versions.
- [Architecture](docs/ENGINEER-ARCHITECTURE.md), [calculations](docs/ENGINEER-CALCULATIONS.md), [channel inventory](docs/LMU-CHANNEL-INVENTORY.md), [API/tool research](docs/ENGINEER-RESEARCH.md).

Private receiver profiles, saved PC identities, credentials, paired driver packages, proprietary SDK headers and test recordings are excluded from this repository and release packages.

## License and upstream

Based on [Swizzjack/lmu-pitwall](https://github.com/Swizzjack/lmu-pitwall), under its [MIT license](LICENSE), with upstream history and attribution retained. The [original upstream README](docs/UPSTREAM-README.md) documents the inherited application. Pitwall Engineer is a separate project from other products named LMU Engineer or LMU Pitwall.
