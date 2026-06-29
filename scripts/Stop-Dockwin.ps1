#Requires -Version 5.1
<#
.SYNOPSIS
    Stop the dockwin engine: stop dockerd and (optionally) shut down the distro.

.DESCRIPTION
    Gracefully stops dockerd inside the dedicated "dockwin" WSL2 distro. By
    default the distro VM is left running (so a subsequent start is fast);
    pass -Terminate to also terminate the distro and free its memory.

    This does NOT touch the named-pipe relay (\\.\pipe\dockwin_engine), which is
    hosted by the dockwin-core process; stop that via `dockwin down` or the GUI.

    NOTE: autostart is via systemd (`systemctl enable docker`). Stopping the
    service here only affects the CURRENT boot -- next time the distro starts,
    systemd will autostart dockerd again. To stop dockerd starting on boot,
    disable the unit:  wsl -d dockwin -u root -- systemctl disable docker

    Idempotent: safe to run when dockerd/the distro are already stopped.

.PARAMETER DistroName
    Distro to act on (default: dockwin).

.PARAMETER Terminate
    Also terminate the distro VM (`wsl --terminate`) after stopping dockerd,
    releasing its RAM. A later Start-Dockwin.ps1 / `dockwin up` reboots it.

.EXAMPLE
    ./Stop-Dockwin.ps1
    ./Stop-Dockwin.ps1 -Terminate
#>
[CmdletBinding()]
param(
    [string]$DistroName = 'dockwin',
    [switch]$Terminate
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Step  { param([string]$m) Write-Host "==> $m" -ForegroundColor Cyan }
function Write-Ok    { param([string]$m) Write-Host "    $m" -ForegroundColor Green }
function Write-Warn2 { param([string]$m) Write-Host "    $m" -ForegroundColor Yellow }

function Test-DistroExists {
    param([string]$Name)
    # `wsl -l -q` output is UTF-16; normalize and strip NULs/whitespace.
    $list = (& wsl.exe --list --quiet) 2>$null
    if (-not $list) { return $false }
    $names = $list -split "`r?`n" | ForEach-Object { ($_ -replace "`0", '').Trim() } |
             Where-Object { $_ -ne '' }
    return ($names -contains $Name)
}

function Test-DistroRunning {
    param([string]$Name)
    # `wsl -l --running -q` lists only running distros (UTF-16 output).
    $list = (& wsl.exe --list --running --quiet) 2>$null
    if (-not $list) { return $false }
    $names = $list -split "`r?`n" | ForEach-Object { ($_ -replace "`0", '').Trim() } |
             Where-Object { $_ -ne '' }
    return ($names -contains $Name)
}

Write-Host ''
Write-Host "dockwin stop  --  engine distro '$DistroName'" -ForegroundColor White
Write-Host '-----------------------------------------------------'

if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    Write-Warn2 'wsl.exe not found; nothing to stop.'
    return
}
if (-not (Test-DistroExists -Name $DistroName)) {
    Write-Warn2 "distro '$DistroName' is not registered; nothing to stop."
    return
}
if (-not (Test-DistroRunning -Name $DistroName)) {
    Write-Ok "distro '$DistroName' is already stopped."
    Write-Host ''
    Write-Host 'dockwin engine stopped.' -ForegroundColor Green
    Write-Host ''
    return
}

# --- 1. Stop dockerd gracefully --------------------------------------------
Write-Step 'Stopping dockerd'
$stopCmd = @'
if [ -d /run/systemd/system ]; then
    systemctl stop docker.socket docker.service 2>/dev/null || systemctl stop docker 2>/dev/null || true
else
    service docker stop 2>/dev/null || true
fi
'@
& wsl.exe -d $DistroName -u root -- bash -lc $stopCmd 2>&1 | Out-Null
Write-Ok 'dockerd stopped (systemd will restart it on next boot unless disabled).'

# --- 2. Optionally terminate the distro VM ---------------------------------
if ($Terminate) {
    Write-Step "Terminating distro '$DistroName' to free memory"
    & wsl.exe --terminate $DistroName 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "distro '$DistroName' terminated."
    } else {
        Write-Warn2 "terminate returned $LASTEXITCODE; the distro may already be down."
    }
} else {
    Write-Warn2 'distro VM left running (fast restart). Use -Terminate to free its RAM.'
}

Write-Host ''
Write-Host 'dockwin engine stopped.' -ForegroundColor Green
Write-Host ''
