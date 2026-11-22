param([string]$ReceiverDirectory, [string]$BuiltUiDirectory)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (!$ReceiverDirectory) { $ReceiverDirectory = Join-Path $repoRoot 'dist/Pitwall-Engineer' }
$receiverRoot = (Resolve-Path -LiteralPath $ReceiverDirectory).Path
if (!(Test-Path -LiteralPath (Join-Path $receiverRoot 'lmu-engineer.exe'))) { throw 'Receiver directory must contain lmu-engineer.exe' }
Push-Location $repoRoot
try {
    if (!$BuiltUiDirectory) {
        & npm --prefix dashboard run build
        if ($LASTEXITCODE -ne 0) { throw 'Dashboard build failed; installed UI was not changed' }
        $BuiltUiDirectory = Join-Path $repoRoot 'dashboard/dist'
    }
    $sourceRoot = (Resolve-Path -LiteralPath $BuiltUiDirectory).Path
    if (!(Test-Path -LiteralPath (Join-Path $sourceRoot 'index.html'))) { throw 'Built UI must contain index.html' }
    $uiRoot = Join-Path $receiverRoot 'ui'
    New-Item -ItemType Directory -Path $uiRoot -Force | Out-Null
    # Retain old hashed assets for tabs still running the previous UI. Publish
    # the index last so it never refers to files that have not been copied yet.
    Get-ChildItem -LiteralPath $sourceRoot -Recurse -File | Where-Object FullName -ne (Join-Path $sourceRoot 'index.html') | ForEach-Object {
        $relative = $_.FullName.Substring($sourceRoot.Length + 1)
        $target = Join-Path $uiRoot $relative
        New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $target -Force
    }
    $index = Join-Path $uiRoot 'index.html'
    $temporary = Join-Path $uiRoot ('index-' + [guid]::NewGuid().ToString('N') + '.tmp')
    Copy-Item -LiteralPath (Join-Path $sourceRoot 'index.html') -Destination $temporary
    if (Test-Path -LiteralPath $index) { [System.IO.File]::Replace($temporary, $index, [NullString]::Value) }
    else { [System.IO.File]::Move($temporary, $index) }
    Write-Output 'UI updated. Refresh the receiver dashboard. Telemetry collection continues.'
} finally { Pop-Location }
