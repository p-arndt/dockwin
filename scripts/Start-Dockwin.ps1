#Requires -Version 5.1
<#
.SYNOPSIS
    Start the dockwin engine distro and bring dockerd up.

.DESCRIPTION
    Boots the dedicated "dockwin" WSL2 distro (which, via /etc/wsl.conf
    systemd=true + `systemctl enable docker`, autostarts dockerd on its UNIX
    socket /var/run/docker.sock) and verifies the daemon is reachable.

    Wiring reminder (architecture's CHOSEN method): Windows talks to dockerd
    through the named-pipe relay \\.\pipe\dockwin_engine, which is hosted by the
    dockwin-core process (started via `dockwin up` or the GUI), NOT by this
    script. This script only ensures the distro + dockerd are running; the
    `docker --context dockwin ...` endpoint additionally needs that relay.

    FALLBACK wiring: if the distro was provisioned with -EnableTcpFallback,
    dockerd also listens on tcp://127.0.0.1:2375 (loopback only, insecure) and
    is reachable via WSL2 localhost-forwarding without the relay.

    Idempotent: safe to run when the distro/dockerd are already up.

.PARAMETER DistroName
    Distro to start (default: dockwin).

.PARAMETER TimeoutSeconds
    How long to wait for dockerd to become reachable (default: 60).

.EXAMPLE
    ./Start-Dockwin.ps1
#>
[CmdletBinding()]
param(
    [string]$DistroName = 'dockwin',
    [int]$TimeoutSeconds = 60
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Step  { param([string]$m) Write-Host "==> $m" -ForegroundColor Cyan }
function Write-Ok    { param([string]$m) Write-Host "    $m" -ForegroundColor Green }
function Write-Warn2 { param([string]$m) Write-Host "    $m" -ForegroundColor Yellow }
function Fail        { param([string]$m) Write-Host "ERROR: $m" -ForegroundColor Red; exit 1 }

function Test-DistroExists {
    param([string]$Name)
    # `wsl -l -q` output is UTF-16; normalize and strip NULs/whitespace.
    $list = (& wsl.exe --list --quiet) 2>$null
    if (-not $list) { return $false }
    $names = $list -split "`r?`n" | ForEach-Object { ($_ -replace "`0", '').Trim() } |
             Where-Object { $_ -ne '' }
    return ($names -contains $Name)
}

Write-Host ''
Write-Host "dockwin start  --  engine distro '$DistroName'" -ForegroundColor White
Write-Host '-----------------------------------------------------'

if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    Fail 'wsl.exe not found. Install WSL2 first:  wsl --install'
}
if (-not (Test-DistroExists -Name $DistroName)) {
    Fail "distro '$DistroName' is not registered. Run scripts/Install-Dockwin.ps1 first."
}

# --- 1. Boot the distro -----------------------------------------------------
# Running any command in the distro triggers WSL to boot it (and, with
# systemd=true in wsl.conf, to start systemd which autostarts dockerd).
Write-Step "Booting distro '$DistroName'"
& wsl.exe -d $DistroName -u root -- true 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { Fail "could not start distro '$DistroName'." }
Write-Ok 'distro is running.'

# --- 2. Ensure dockerd is started -------------------------------------------
# systemd should have started docker.service already; nudge it just in case
# (e.g. after a previous `Stop-Dockwin.ps1` that stopped the service).
Write-Step 'Ensuring dockerd is started'
$startCmd = @'
if [ -d /run/systemd/system ]; then
    systemctl is-active --quiet docker || systemctl start docker
else
    service docker status >/dev/null 2>&1 || service docker start
fi
'@
& wsl.exe -d $DistroName -u root -- bash -lc $startCmd 2>&1 | Out-Null

# --- 3. Wait for dockerd to be reachable on its unix socket -----------------
Write-Step 'Waiting for dockerd to become reachable'
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$ready = $false
$serverVer = ''
while ((Get-Date) -lt $deadline) {
    $serverVer = (& wsl.exe -d $DistroName -u root -- bash -lc 'docker version --format "{{.Server.Version}}" 2>/dev/null') 2>$null
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($serverVer)) {
        $ready = $true
        break
    }
    Start-Sleep -Seconds 1
}

if (-not $ready) {
    Write-Warn2 "dockerd not reachable after ${TimeoutSeconds}s."
    Write-Warn2 "diagnose with:  wsl -d $DistroName -u root -- journalctl -u docker --no-pager -n 50"
    Fail 'engine did not come up.'
}
Write-Ok "dockerd is up (server $($serverVer.Trim()))."

Write-Host ''
Write-Host 'dockwin engine started.' -ForegroundColor Green
Write-Host "  engine   : dockerd on unix:///var/run/docker.sock inside '$DistroName'" -ForegroundColor Gray
Write-Host '  windows  : start the named-pipe relay via `dockwin up` / the GUI, then' -ForegroundColor Gray
Write-Host "             docker --context $DistroName ps" -ForegroundColor Gray
Write-Host ''
