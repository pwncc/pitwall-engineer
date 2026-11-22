param([int]$ReceiverPort = 7437, [int]$Port = 5173)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ($ReceiverPort -lt 1 -or $ReceiverPort -gt 65535 -or $Port -lt 1 -or $Port -gt 65535) { throw 'Invalid port' }
$null = Invoke-RestMethod "http://127.0.0.1:$ReceiverPort/api/state" -TimeoutSec 5
$previousReceiverPort = $env:ENGINEER_HTTP_PORT
$previousDevPort = $env:ENGINEER_DEV_PORT
Push-Location $repoRoot
try {
    if (!(Test-Path -LiteralPath 'dashboard/node_modules/vite')) {
        & npm --prefix dashboard ci
        if ($LASTEXITCODE -ne 0) { throw 'Dependency installation failed' }
    }
    $env:ENGINEER_HTTP_PORT = "$ReceiverPort"
    $env:ENGINEER_DEV_PORT = "$Port"
    Write-Output "Dashboard: http://127.0.0.1:$Port"
    Write-Output "Receiver: http://127.0.0.1:$ReceiverPort"
    & npm --prefix dashboard run dev
    if ($LASTEXITCODE -ne 0) { throw 'Development server stopped with an error' }
} finally {
    $env:ENGINEER_HTTP_PORT = $previousReceiverPort
    $env:ENGINEER_DEV_PORT = $previousDevPort
    Pop-Location
}
