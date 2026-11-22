param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$buildStamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$engineerVersion = (Get-Content -LiteralPath (Join-Path $repoRoot 'ENGINEER-VERSION') -Raw).Trim()
$bundleRoot = Join-Path $repoRoot "dist/Pitwall-Engineer-$engineerVersion-$buildStamp"
$cacheRoot = Join-Path $repoRoot 'dist/.build-cache'
New-Item -ItemType Directory -Path $bundleRoot,$cacheRoot -Force | Out-Null
$previousRustFlags = $env:RUSTFLAGS
$previousDuckLibrary = $env:LMU_DUCKDB_DLL
Push-Location $repoRoot
try {
    $duckDll = Join-Path $cacheRoot 'duckdb-1.4.0.dll'
    $expectedDuckHash = 'D36068E85CD0261F5A6E281537742E6FAA5F2B4BF3B290A68D71CC1250617B33'
    if (!(Test-Path -LiteralPath $duckDll)) {
        $archive = Join-Path $cacheRoot 'duckdb-1.4.0.zip'
        Invoke-WebRequest 'https://github.com/duckdb/duckdb/releases/download/v1.4.0/libduckdb-windows-amd64.zip' -OutFile $archive
        $extractPath = Join-Path $cacheRoot "duckdb-$buildStamp"
        Expand-Archive -LiteralPath $archive -DestinationPath $extractPath
        Copy-Item -LiteralPath (Join-Path $extractPath 'duckdb.dll') -Destination $duckDll
    }
    if ((Get-FileHash -LiteralPath $duckDll -Algorithm SHA256).Hash -ne $expectedDuckHash) { throw 'DuckDB library hash mismatch' }
    Copy-Item -LiteralPath $duckDll -Destination (Join-Path $bundleRoot 'duckdb.dll')
    $env:LMU_DUCKDB_DLL = $duckDll
    $env:RUSTFLAGS = (($previousRustFlags + ' -C target-feature=+crt-static').Trim())
    & npm --prefix dashboard ci
    if ($LASTEXITCODE -ne 0) { throw 'Dependency installation failed' }
    & npm --prefix dashboard run build
    if ($LASTEXITCODE -ne 0) { throw 'Dashboard build failed' }
    Push-Location dashboard
    try { & npx eslint src/engineer src/App.tsx; if ($LASTEXITCODE -ne 0) { throw 'Engineer UI lint failed' } } finally { Pop-Location }
    if (!$SkipTests) {
        & cargo test --release --manifest-path bridge/Cargo.toml --lib --test endurance --test network
        if ($LASTEXITCODE -ne 0) { throw 'Backend checks failed' }
    }
    & cargo build --manifest-path bridge/Cargo.toml --release --bin lmu-engineer --bin lmu-agent --locked
    if ($LASTEXITCODE -ne 0) { throw 'Windows release build failed' }
    Copy-Item -LiteralPath 'bridge/target/release/lmu-engineer.exe','bridge/target/release/lmu-agent.exe','LICENSE','README-ENGINEER.md','START-HERE.txt' -Destination $bundleRoot
    Copy-Item -LiteralPath 'dashboard/dist' -Destination (Join-Path $bundleRoot 'ui') -Recurse
    Copy-Item -LiteralPath 'docs/ENGINEER-CALCULATIONS.md','docs/ENGINEER-RESEARCH.md','docs/ENGINEER-VALIDATION.md','docs/ENGINEER-ARCHITECTURE.md','docs/ENGINEER-DEVELOPMENT.md','docs/LMU-CHANNEL-INVENTORY.md' -Destination $bundleRoot
    $notices = Join-Path $bundleRoot 'THIRD-PARTY'
    & python scripts/collect-engineer-licenses.py $notices
    if ($LASTEXITCODE -ne 0) { throw 'Dependency notice collection failed' }
    Invoke-WebRequest 'https://raw.githubusercontent.com/duckdb/duckdb/v1.4.0/LICENSE' -OutFile (Join-Path $notices 'DuckDB-LICENSE')
    Get-ChildItem -LiteralPath $bundleRoot -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
        $relative = $_.FullName.Substring($bundleRoot.Length + 1)
        '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(),$relative
    } | Set-Content -LiteralPath (Join-Path $bundleRoot 'SHA256SUMS.txt') -Encoding utf8
    $zipPath = Join-Path $repoRoot "dist/Pitwall-Engineer-$engineerVersion-$buildStamp.zip"
    Compress-Archive -Path (Join-Path $bundleRoot '*') -DestinationPath $zipPath
    Write-Output "Bundle: $bundleRoot"
    Write-Output "ZIP: $zipPath"
} finally {
    $env:RUSTFLAGS = $previousRustFlags
    $env:LMU_DUCKDB_DLL = $previousDuckLibrary
    Pop-Location
}
