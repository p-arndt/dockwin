#Requires -Version 5.1
<#
.SYNOPSIS
    Cleanly tear down the dockwin engine: unregister the WSL2 distro and remove
    the Windows-side docker context(s).

.DESCRIPTION
    Reverses Install-Dockwin.ps1. By default this PERMANENTLY deletes the
    distro's ext4.vhdx (`wsl --unregister`) -- there is no undo. Use -Backup to
    export the distro to a .tar first.

.PARAMETER DistroName
    Distro to remove (default: dockwin).

.PARAMETER Backup
    Export the distro to a .tar before unregistering.

.PARAMETER BackupPath
    Where to write the backup tar (default: %USERPROFILE%\dockwin-backup-<date>.tar).

.PARAMETER Force
    Skip the interactive confirmation prompt.

.EXAMPLE
    ./Uninstall-Dockwin.ps1
    ./Uninstall-Dockwin.ps1 -Backup -Force
#>
[CmdletBinding()]
param(
    [string]$DistroName = 'dockwin',
    [switch]$Backup,
    [string]$BackupPath,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Step  { param([string]$m) Write-Host "==> $m" -ForegroundColor Cyan }
function Write-Ok    { param([string]$m) Write-Host "    $m" -ForegroundColor Green }
function Write-Warn2 { param([string]$m) Write-Host "    $m" -ForegroundColor Yellow }

function Test-DistroExists {
    param([string]$Name)
    $list = (& wsl.exe --list --quiet) 2>$null
    if (-not $list) { return $false }
    $names = $list -split "`r?`n" | ForEach-Object { ($_ -replace "`0", '').Trim() } |
             Where-Object { $_ -ne '' }
    return ($names -contains $Name)
}

Write-Host ''
Write-Host "dockwin uninstaller  --  removing distro '$DistroName'" -ForegroundColor White
Write-Host '-----------------------------------------------------'

if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    Write-Warn2 'wsl.exe not found; nothing to unregister.'
}

# --- 1. Remove Windows docker context(s) -----------------------------------
Write-Step 'Removing Windows docker context(s)'
if (Get-Command docker.exe -ErrorAction SilentlyContinue) {
    $contexts = (& docker.exe context ls --format '{{.Name}}') 2>$null
    $current  = (& docker.exe context show) 2>$null
    foreach ($ctx in @($DistroName, "$DistroName-tcp")) {
        if ($contexts -split "`r?`n" -contains $ctx) {
            # Can't remove the in-use context; switch to default first.
            if ($current -and $current.Trim() -eq $ctx) {
                & docker.exe context use default 2>&1 | Out-Null
            }
            & docker.exe context rm -f $ctx 2>&1 | Out-Null
            Write-Ok "removed docker context '$ctx'"
        }
    }
} else {
    Write-Warn2 'docker.exe not found; skipping context cleanup.'
}

# --- 2. Confirm destructive unregister -------------------------------------
if (-not (Test-DistroExists -Name $DistroName)) {
    Write-Warn2 "distro '$DistroName' not registered; nothing to unregister."
    Write-Host ''
    Write-Host 'dockwin uninstall complete (context-only).' -ForegroundColor Green
    return
}

Write-Step "Tearing down WSL2 distro '$DistroName'"
Write-Warn2 'wsl --unregister PERMANENTLY deletes the distro vhdx. There is no undo.'

if (-not $Force) {
    $answer = Read-Host "Type the distro name '$DistroName' to confirm deletion"
    if ($answer -ne $DistroName) {
        Write-Host 'Aborted; nothing was deleted.' -ForegroundColor Yellow
        return
    }
}

# --- 3. Optional backup ----------------------------------------------------
if ($Backup) {
    if (-not $BackupPath) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $BackupPath = Join-Path $env:USERPROFILE "dockwin-backup-$stamp.tar"
    }
    Write-Step "Exporting backup -> $BackupPath"
    & wsl.exe --export $DistroName $BackupPath 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) { Write-Ok 'backup written.' }
    else { Write-Warn2 'backup export failed; continuing with unregister.' }
}

# --- 4. Stop + unregister --------------------------------------------------
& wsl.exe --terminate $DistroName 2>&1 | Out-Null
& wsl.exe --unregister $DistroName 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    Write-Ok "unregistered '$DistroName' (vhdx deleted)."
} else {
    Write-Warn2 "unregister returned $LASTEXITCODE; the distro may already be gone."
}

Write-Host ''
Write-Host 'dockwin uninstall complete.' -ForegroundColor Green
Write-Host ''
