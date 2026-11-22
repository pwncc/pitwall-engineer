param(
    [Parameter(Mandatory=$true)][string]$BundleDirectory,
    [string]$ReceiverDirectory,
    [string]$DataDirectory = (Join-Path $env:LOCALAPPDATA 'LMUEngineer'),
    [int]$HttpPort = 7437,
    [int]$IngestPort = 7443,
    [switch]$NoTray
)
$ErrorActionPreference = 'Stop'
function Get-EngineerFileHash([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $hash = [System.Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($hash.ComputeHash($stream)).Replace('-', '') }
    finally { $hash.Dispose(); $stream.Dispose() }
}
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (!$ReceiverDirectory) { $ReceiverDirectory = Join-Path $repoRoot 'dist/Pitwall-Engineer' }
$bundle = (Resolve-Path -LiteralPath $BundleDirectory).Path
$receiverRoot = (Resolve-Path -LiteralPath $ReceiverDirectory).Path
$profileRoot = (Resolve-Path -LiteralPath $DataDirectory).Path
$installedExe = Join-Path $receiverRoot 'lmu-engineer.exe'
$replacement = Join-Path $bundle 'lmu-engineer.exe'
if ($bundle -eq $receiverRoot) { throw 'Build the replacement in a separate bundle directory' }
if (!(Test-Path -LiteralPath $replacement) -or !(Test-Path -LiteralPath (Join-Path $bundle 'ui/index.html'))) { throw 'Incomplete replacement bundle' }
$certificate = Join-Path $profileRoot 'receiver.der'
if (!(Test-Path -LiteralPath $certificate)) { throw 'Select the existing receiver data directory to preserve pairing' }
$certificateHash = Get-EngineerFileHash $certificate
$agent = Join-Path $receiverRoot 'lmu-agent.exe'
$agentHash = Get-EngineerFileHash $agent
$url = "http://127.0.0.1:$HttpPort/api/state"
$before = Invoke-RestMethod $url -TimeoutSec 10
if ([System.IO.Path]::GetFullPath($before.data_directory).TrimEnd('\') -ne $profileRoot.TrimEnd('\')) { throw 'Running receiver uses a different data directory' }
$processes = @(Get-Process -Name lmu-engineer -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installedExe })
if ($processes.Count -ne 1) { throw 'Expected exactly one running receiver at the selected executable path' }
$backup = Join-Path $receiverRoot ('.receiver-backups/' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $backup -Force | Out-Null
Copy-Item -LiteralPath $installedExe -Destination (Join-Path $backup 'lmu-engineer.exe')
$index = Join-Path $receiverRoot 'ui/index.html'
$hadIndex = Test-Path -LiteralPath $index
if ($hadIndex) { Copy-Item -LiteralPath $index -Destination (Join-Path $backup 'index.html') }
$receiverArgs = @('--data',('"'+$profileRoot+'"'),'--http-port',"$HttpPort",'--ingest-port',"$IngestPort",'--no-browser')
if ($NoTray) { $receiverArgs += '--no-tray' }
$started = $null
try {
    Write-Output 'Restarting receiver. Existing driver agents keep recording and reconnect automatically.'
    Stop-Process -Id $processes[0].Id
    Wait-Process -Id $processes[0].Id -ErrorAction SilentlyContinue
    $null = $processes[0].WaitForExit(10000)
    $processes[0].Dispose()
    # Windows can retain the old image mapping after process exit. Retire the
    # old file by rename instead of overwriting that mapped image in place.
    [System.IO.File]::Move($installedExe, (Join-Path $backup 'retired-lmu-engineer.exe'))
    Copy-Item -LiteralPath $replacement -Destination $installedExe
    & (Join-Path $PSScriptRoot 'update-engineer-ui.ps1') -ReceiverDirectory $receiverRoot -BuiltUiDirectory (Join-Path $bundle 'ui')
    $started = Start-Process -FilePath $installedExe -ArgumentList $receiverArgs -WindowStyle Hidden -PassThru
    $deadline = [datetime]::UtcNow.AddSeconds(180)
    $after = $null
    while ([datetime]::UtcNow -lt $deadline) {
        if ($started.HasExited) { throw 'Replacement receiver exited during startup' }
        try { $after = Invoke-RestMethod $url -TimeoutSec 2; break } catch { Start-Sleep -Milliseconds 250 }
    }
    if (!$after) { throw 'Replacement receiver did not become ready within 180 seconds' }
    if ($after.event_id -ne $before.event_id -or $after.version -ne $before.version) { throw 'Receiver changed the event identity or collector protocol' }
    if ((Get-EngineerFileHash $certificate) -ne $certificateHash) { throw 'Receiver certificate changed' }
    if ((Get-EngineerFileHash $agent) -ne $agentHash) { throw 'Stock collector executable changed' }
    foreach ($source in $before.sources) {
        if (!($after.sources | Where-Object source_id -eq $source.source_id)) { throw 'A driver pairing was not restored' }
    }
    Write-Output 'Receiver updated. Existing driver packages and pairing are unchanged.'
} catch {
    $failure = $_
    if ($started -and !$started.HasExited) { Stop-Process -Id $started.Id; Wait-Process -Id $started.Id -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $installedExe) {
        [System.IO.File]::Move($installedExe, (Join-Path $backup 'failed-lmu-engineer.exe'))
    }
    Copy-Item -LiteralPath (Join-Path $backup 'lmu-engineer.exe') -Destination $installedExe
    if ($hadIndex) { Copy-Item -LiteralPath (Join-Path $backup 'index.html') -Destination $index -Force }
    elseif (Test-Path -LiteralPath $index) { Remove-Item -LiteralPath $index -Force }
    Start-Process -FilePath $installedExe -ArgumentList $receiverArgs -WindowStyle Hidden | Out-Null
    throw "Update failed; previous receiver restored. $failure"
}
