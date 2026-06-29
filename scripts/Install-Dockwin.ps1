#Requires -Version 5.1
<#
.SYNOPSIS
    Provision the dedicated "dockwin" WSL2 distro and wire it to Windows.

.DESCRIPTION
    End-to-end, IDEMPOTENT installer for the dockwin engine host:

      1. Preflight: ensure WSL2 is present and recent enough for systemd=true
         (`wsl --version` / `wsl --update`).
      2. Import a minimal Ubuntu 24.04 rootfs as a NEW distro named "dockwin"
         (separate from any user Ubuntu) via `wsl --import ... --version 2`.
         If a .tar.gz is given it is decompressed to a plain .tar first
         (gzip import fails with "Incorrect function." on older WSL builds).
      3. Drop /etc/wsl.conf (systemd + autostart config) into the distro.
      4. Run distro/provision-inside.sh inside the distro to install pinned
         dockerd, set iptables-legacy, systemd cgroup driver, enable autostart.
      5. `wsl --shutdown` to apply wsl.conf, restart the distro, verify dockerd.
      6. Wire Windows -> engine using the CHOSEN method: a Windows docker
         CONTEXT named "dockwin" pointing at the named pipe
         npipe:////./pipe/dockwin_engine. The pipe SERVER (the per-connection
         wsl.exe+socat relay) is hosted by the dockwin-core process; start it
         with `dockwin up` or the GUI. This script only registers the context.

         FALLBACK wiring (only when -EnableTcpFallback is passed): provisions
         dockerd to also listen on tcp://127.0.0.1:2375 (loopback only) and
         registers a `dockwin-tcp` context via WSL2 localhost-forwarding. This
         path is UNAUTHENTICATED and reachable by any local process / other
         WSL distro -- explicitly insecure, never the default, never 0.0.0.0.

.PARAMETER RootfsPath
    Path to the Ubuntu 24.04 WSL rootfs tarball (.tar or .tar.gz). If omitted
    the script downloads the official cloud-images noble WSL rootfs.

.PARAMETER InstallDir
    Directory that will hold the distro's ext4.vhdx.
    Default: %LOCALAPPDATA%\dockwin\distro

.PARAMETER EnableTcpFallback
    Also enable the insecure loopback TCP (127.0.0.1:2375) fallback wiring.

.EXAMPLE
    ./Install-Dockwin.ps1
    ./Install-Dockwin.ps1 -RootfsPath C:\images\ubuntu-24.04-wsl.rootfs.tar
#>
[CmdletBinding()]
param(
    [string]$RootfsPath,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'dockwin\distro'),
    [string]$DistroName = 'dockwin',
    [switch]$EnableTcpFallback
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Named pipe the dockwin-core relay serves; the docker context points here.
$PipeContextHost = 'npipe:////./pipe/dockwin_engine'
$TcpContextHost  = 'tcp://localhost:2375'
$ScriptRoot      = Split-Path -Parent $MyInvocation.MyCommand.Path
$DistroDir       = Split-Path -Parent $ScriptRoot   # repo root
$DistroDir       = Join-Path $DistroDir 'distro'
$WslConfSrc      = Join-Path $DistroDir 'wsl.conf'
$ProvisionSrc    = Join-Path $DistroDir 'provision-inside.sh'

# Default official Ubuntu 24.04 (noble) WSL rootfs.
$DefaultRootfsUrl = 'https://cloud-images.ubuntu.com/wsl/noble/current/ubuntu-noble-wsl-amd64-wsl.rootfs.tar.gz'

# ---------------------------------------------------------------------------
function Write-Step  { param([string]$m) Write-Host "==> $m" -ForegroundColor Cyan }
function Write-Ok    { param([string]$m) Write-Host "    $m" -ForegroundColor Green }
function Write-Warn2 { param([string]$m) Write-Host "    $m" -ForegroundColor Yellow }
function Fail        { param([string]$m) Write-Host "ERROR: $m" -ForegroundColor Red; exit 1 }

function Invoke-Wsl {
    # Run wsl.exe and throw on non-zero exit. Returns captured stdout text.
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)
    $out = & wsl.exe @Args 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "wsl.exe $($Args -join ' ') failed ($LASTEXITCODE): $out"
    }
    return $out
}

function Test-DistroExists {
    param([string]$Name)
    # `wsl -l -q` output is UTF-16; normalize and strip NULs/whitespace.
    $list = (& wsl.exe --list --quiet) 2>$null
    if (-not $list) { return $false }
    $names = $list -split "`r?`n" | ForEach-Object { ($_ -replace "`0", '').Trim() } |
             Where-Object { $_ -ne '' }
    return ($names -contains $Name)
}

function Copy-IntoDistro {
    # Copy a host file into the distro at $DestPath, normalizing CRLF -> LF so
    # shell/conf files don't break under Linux.
    param([string]$Name, [string]$SrcPath, [string]$DestPath, [string]$Mode = '0644')
    if (-not (Test-Path $SrcPath)) { Fail "missing source file: $SrcPath" }
    $winPath = (Resolve-Path $SrcPath).Path
    # Translate the Windows path into the distro's /mnt view.
    $drive = $winPath.Substring(0, 1).ToLower()
    $rest  = $winPath.Substring(2) -replace '\\', '/'
    $mnt   = "/mnt/$drive$rest"
    Invoke-Wsl -d $DistroName -u root -- bash -lc "tr -d '\r' < '$mnt' > '$DestPath' && chmod $Mode '$DestPath'" | Out-Null
    Write-Ok "placed $Name -> $DestPath"
}

# ---------------------------------------------------------------------------
Write-Host ''
Write-Host 'dockwin installer  --  dedicated WSL2 Docker engine' -ForegroundColor White
Write-Host '-----------------------------------------------------'

# --- 1. Preflight: WSL present + recent ------------------------------------
Write-Step 'Preflight: checking WSL2'
if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    Fail 'wsl.exe not found. Install WSL2 first:  wsl --install'
}
$verText = (& wsl.exe --version) 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($verText)) {
    Write-Warn2 'Old "inbox" WSL detected (no --version). systemd=true needs WSL >= 2.1.5.'
    Write-Warn2 'Attempting `wsl --update` ...'
    & wsl.exe --update 2>&1 | Out-Null
} else {
    Write-Ok ((($verText -split "`r?`n") | Select-Object -First 1).Trim())
}
& wsl.exe --set-default-version 2 2>&1 | Out-Null

# --- 2. Resolve / fetch rootfs ---------------------------------------------
Write-Step 'Resolving Ubuntu rootfs'
$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("dockwin-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

if (-not $RootfsPath) {
    $RootfsPath = Join-Path $tmpDir 'ubuntu-noble-wsl.rootfs.tar.gz'
    Write-Ok "downloading $DefaultRootfsUrl"
    try {
        $oldPref = $ProgressPreference; $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $DefaultRootfsUrl -OutFile $RootfsPath -UseBasicParsing
        $ProgressPreference = $oldPref
    } catch {
        Fail "rootfs download failed: $($_.Exception.Message)`n    Pass -RootfsPath to a local tar instead."
    }
}
if (-not (Test-Path $RootfsPath)) { Fail "rootfs not found: $RootfsPath" }

# Decompress .tar.gz -> .tar (gz import fails on older WSL builds).
$importTar = (Resolve-Path $RootfsPath).Path
if ($importTar -match '\.t?gz$' -or $importTar -match '\.tar\.gz$') {
    Write-Ok 'decompressing .tar.gz -> .tar (avoids "Incorrect function." import bug)'
    $plainTar = Join-Path $tmpDir 'rootfs.tar'
    $inStream  = [System.IO.File]::OpenRead($importTar)
    $gz        = New-Object System.IO.Compression.GZipStream($inStream, [System.IO.Compression.CompressionMode]::Decompress)
    $outStream = [System.IO.File]::Create($plainTar)
    try { $gz.CopyTo($outStream) } finally { $gz.Dispose(); $inStream.Dispose(); $outStream.Dispose() }
    $importTar = $plainTar
}
Write-Ok "rootfs ready: $importTar"

# --- 3. Import the distro (idempotent) -------------------------------------
Write-Step "Importing WSL2 distro '$DistroName'"
if (Test-DistroExists -Name $DistroName) {
    Write-Warn2 "distro '$DistroName' already exists; skipping import (re-provisioning in place)."
} else {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Invoke-Wsl --import $DistroName $InstallDir $importTar --version 2 | Out-Null
    Write-Ok "imported into $InstallDir"
}

# --- 4. Place wsl.conf -----------------------------------------------------
Write-Step 'Configuring /etc/wsl.conf'
# Ensure the distro is running so we can write into it.
Invoke-Wsl -d $DistroName -u root -- true | Out-Null
Copy-IntoDistro -Name 'wsl.conf' -SrcPath $WslConfSrc -DestPath '/etc/wsl.conf' -Mode '0644'

# Apply wsl.conf (systemd) by cycling the distro.
Write-Ok 'applying wsl.conf (wsl --shutdown to enable systemd)'
& wsl.exe --shutdown 2>&1 | Out-Null
Start-Sleep -Seconds 3
Invoke-Wsl -d $DistroName -u root -- true | Out-Null

# --- 5. Provision inside ---------------------------------------------------
Write-Step 'Provisioning Docker Engine inside the distro'
Copy-IntoDistro -Name 'provision-inside.sh' -SrcPath $ProvisionSrc -DestPath '/usr/local/sbin/dockwin-provision.sh' -Mode '0755'

$envPrefix = ''
if ($EnableTcpFallback) {
    Write-Warn2 'TCP fallback enabled: dockerd will ALSO bind 127.0.0.1:2375 (INSECURE).'
    $envPrefix = 'DOCKWIN_ENABLE_TCP=1 '
}
Write-Ok 'running provision-inside.sh (this installs dockerd; may take a few minutes)...'
& wsl.exe -d $DistroName -u root -- bash -lc "$envPrefix/usr/local/sbin/dockwin-provision.sh"
if ($LASTEXITCODE -ne 0) { Fail 'in-distro provisioning failed (see output above).' }

# --- 6. Verify dockerd via the distro --------------------------------------
Write-Step 'Verifying dockerd'
$srvVer = (& wsl.exe -d $DistroName -u root -- bash -lc 'docker version --format "{{.Server.Version}}" 2>/dev/null') 2>&1
if ($LASTEXITCODE -eq 0 -and $srvVer) {
    Write-Ok "dockerd server version: $($srvVer.Trim())"
} else {
    Write-Warn2 'could not confirm dockerd yet; check: wsl -d dockwin -u root -- journalctl -u docker'
}

# --- 7. Wire Windows docker context(s) -------------------------------------
Write-Step 'Wiring Windows docker context'
$haveDocker = [bool](Get-Command docker.exe -ErrorAction SilentlyContinue)
if ($haveDocker) {
    # Primary: named-pipe context (served by the dockwin-core relay).
    $existing = (& docker.exe context ls --format '{{.Name}}') 2>$null
    if ($existing -split "`r?`n" -contains $DistroName) {
        & docker.exe context update $DistroName --docker "host=$PipeContextHost" 2>&1 | Out-Null
        Write-Ok "updated docker context '$DistroName' -> $PipeContextHost"
    } else {
        & docker.exe context create $DistroName --docker "host=$PipeContextHost" `
            --description 'dockwin engine via named-pipe relay' 2>&1 | Out-Null
        Write-Ok "created docker context '$DistroName' -> $PipeContextHost"
    }

    if ($EnableTcpFallback) {
        $tcpName = "$DistroName-tcp"
        if ($existing -split "`r?`n" -contains $tcpName) {
            & docker.exe context update $tcpName --docker "host=$TcpContextHost" 2>&1 | Out-Null
        } else {
            & docker.exe context create $tcpName --docker "host=$TcpContextHost" `
                --description 'dockwin engine via INSECURE loopback TCP fallback' 2>&1 | Out-Null
        }
        Write-Ok "created INSECURE fallback context '$tcpName' -> $TcpContextHost"
    }
    Write-Warn2 "Use it with:  docker context use $DistroName   (or: docker --context $DistroName ps)"
    Write-Warn2 'The named-pipe relay is hosted by dockwin-core; start it with `dockwin up` or the GUI.'
} else {
    Write-Warn2 'docker.exe (Windows CLI) not found on PATH; skipped context creation.'
    Write-Warn2 "To wire later:  docker context create $DistroName --docker host=$PipeContextHost"
}

# --- cleanup ---------------------------------------------------------------
try { Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue } catch {}

Write-Host ''
Write-Host 'dockwin install complete.' -ForegroundColor Green
Write-Host "  distro     : $DistroName  (vhdx in $InstallDir)" -ForegroundColor Gray
Write-Host "  engine     : dockerd on unix:///var/run/docker.sock (no TCP unless -EnableTcpFallback)" -ForegroundColor Gray
Write-Host "  windows    : docker context '$DistroName' -> $PipeContextHost" -ForegroundColor Gray
Write-Host '  next       : start the relay via `dockwin up` / the GUI, then `docker --context dockwin ps`' -ForegroundColor Gray
Write-Host ''
