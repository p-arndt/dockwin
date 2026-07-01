#!/usr/bin/env bash
#
# provision-inside.sh - run INSIDE the dedicated "dockwin" WSL2 distro as root.
#
# Installs Docker Engine from the PINNED official Docker apt repo (not the
# unpinned get.docker.com convenience script), configures the documented WSL2
# gotchas (iptables-legacy, cgroupfs cgroup driver, cgroup v2), starts dockerd
# via the NON-systemd supervisor (/usr/local/sbin/dockwin-supervise.sh, like
# Docker Desktop), and verifies the daemon comes up on its UNIX socket.
#
# dockerd listens ONLY on /var/run/docker.sock (no TCP) per the architecture's
# chosen wiring. The Windows side reaches it through the named-pipe relay.
#
# FALLBACK wiring (loopback TCP, explicitly insecure): if this script is run
# with DOCKWIN_ENABLE_TCP=1 it ALSO makes dockerd listen on
# tcp://127.0.0.1:2375 (loopback only) so the `dockwin-tcp` docker context can
# reach it via WSL2 localhost-forwarding. Off by default. Never bind 0.0.0.0.
#
# Idempotent: safe to re-run. Designed for Ubuntu 24.04 (noble) rootfs.

set -euo pipefail

log() { printf '\033[1;36m[provision]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[provision][warn]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m[provision][error]\033[0m %s\n' "$*" >&2; exit 1; }

# apt-get update with a proxy-aware failure hint. On a locked-down host the most
# common failure is HTTP 407 (the corporate proxy demands auth apt can't supply),
# which otherwise dies with a bare non-zero exit under `set -e`. Point the user at
# the fix instead of leaving them to decode the raw apt error.
apt_update() {
    if ! apt-get update -y; then
        die "apt-get update failed${PROXY:+ (proxy: ${SAFE_PROXY})}. Behind an \
authenticating proxy this is almost always HTTP 407: supply an authenticated proxy \
with 'dockwin install --proxy http://USER:PASS@HOST:PORT', or point at a local \
no-auth forwarder (e.g. px/cntlm) with 'dockwin install --proxy http://127.0.0.1:3128'."
    fi
}

# Can we open a TCP connection to the proxy in $1? Resolves its host and probes
# the port with bash's /dev/tcp (no curl needed — it isn't installed yet). Used to
# tell a usable proxy from a stale inherited one (e.g. a corporate proxy hostname
# that doesn't resolve off the corporate LAN). Only tests reachability, not auth.
proxy_reachable() {
    local url="$1" hostport host port
    hostport="${url#*://}"      # strip scheme
    hostport="${hostport##*@}"  # strip any user:pass@
    hostport="${hostport%%/*}"  # strip any /path
    host="${hostport%%:*}"
    port="${hostport##*:}"
    [ "$host" = "$port" ] && port=80   # no explicit port
    getent hosts "$host" >/dev/null 2>&1 || return 1
    timeout 5 bash -c "exec 3<>/dev/tcp/${host}/${port}" 2>/dev/null
}

# Does direct (proxy-less) egress to the package mirrors work? Lets us ignore a
# dead inherited proxy when the host actually has a direct route (e.g. at home).
direct_egress() {
    timeout 6 bash -c 'exec 3<>/dev/tcp/archive.ubuntu.com/80' 2>/dev/null && return 0
    timeout 6 bash -c 'exec 3<>/dev/tcp/download.docker.com/443' 2>/dev/null
}

[ "$(id -u)" -eq 0 ] || die "must run as root inside the distro"

ENABLE_TCP="${DOCKWIN_ENABLE_TCP:-0}"
DEBIAN_FRONTEND=noninteractive
export DEBIAN_FRONTEND

# ---------------------------------------------------------------------------
# 0. Sanity: detect distro codename (default to noble / 24.04).
# ---------------------------------------------------------------------------
. /etc/os-release 2>/dev/null || true
DISTRO_ID="${ID:-ubuntu}"
CODENAME="${VERSION_CODENAME:-noble}"
log "base image: ${PRETTY_NAME:-unknown} (id=${DISTRO_ID} codename=${CODENAME})"

if [ "$DISTRO_ID" != "ubuntu" ] && [ "$DISTRO_ID" != "debian" ]; then
    warn "untested base '${DISTRO_ID}'; assuming Debian-family apt layout"
fi

# ---------------------------------------------------------------------------
# 0b. Recover from a previously interrupted run.
#     If an earlier provisioning was killed mid-apt (the app closed, the machine
#     slept, the user cancelled), dpkg can be left in an "interrupted" state and
#     every subsequent apt call refuses to proceed until it is reconfigured.
#     Because this script is idempotent and re-run by `dockwin install`, heal
#     that state up front so a second attempt always completes instead of
#     leaving the engine half-installed (no dockerd binary).
# ---------------------------------------------------------------------------
log "ensuring dpkg/apt state is consistent (recovers from an interrupted run)"
dpkg --configure -a 2>/dev/null || true
apt-get install -f -y 2>/dev/null || true

# ---------------------------------------------------------------------------
# 0c. HTTP(S) proxy resolution.
#     A locked-down host often reaches the internet only through a corporate
#     proxy. WSL injects the Windows proxy into the distro env (http_proxy/
#     HTTP_PROXY), but that value can be WRONG for the current network — e.g. the
#     corporate proxy hostname doesn't resolve when you're off the VPN/LAN, which
#     would make every apt call fail with "Could not resolve <proxy>". So we do
#     NOT blindly trust the inherited proxy: we probe it, and fall back to direct
#     egress when it's dead but the mirrors are directly reachable.
#
#     Resolution:
#       * explicit DOCKWIN_PROXY=<url>       -> use it verbatim
#       * explicit DOCKWIN_PROXY=direct|none -> force proxy-less (direct)
#       * empty + inherited proxy reachable  -> use the inherited proxy
#       * empty + inherited proxy dead but direct egress works -> go direct
#       * empty + no inherited proxy          -> direct
#     A chosen proxy is written to a PERSISTENT apt.conf.d drop-in (so later
#     `dockwin update` apt runs pick it up), exported for the curl calls below,
#     and folded into daemon.json's "proxies" so runtime `docker pull` uses it.
#     Direct mode WRITES a "DIRECT" apt drop-in and unsets the inherited proxy env
#     so apt/curl ignore the stale WSL-injected value.
# ---------------------------------------------------------------------------
EXPLICIT_PROXY="${DOCKWIN_PROXY:-}"
INHERITED_PROXY="${https_proxy:-${http_proxy:-${HTTP_PROXY:-}}}"
NO_PROXY_VAL="${DOCKWIN_NO_PROXY:-${no_proxy:-${NO_PROXY:-localhost,127.0.0.1,::1}}}"
PROXY=""
SAFE_PROXY=""

case "$(printf '%s' "$EXPLICIT_PROXY" | tr '[:upper:]' '[:lower:]')" in
    none|direct|off)
        log "proxy explicitly disabled — using direct egress"
        ;;
    "")
        if [ -n "$INHERITED_PROXY" ]; then
            if proxy_reachable "$INHERITED_PROXY"; then
                PROXY="$INHERITED_PROXY"
                log "inherited proxy is reachable — using it"
            elif direct_egress; then
                warn "inherited proxy '${INHERITED_PROXY}' is unreachable here, but the package mirrors are directly reachable — provisioning WITHOUT a proxy"
            else
                PROXY="$INHERITED_PROXY"
                warn "inherited proxy '${INHERITED_PROXY}' looks unreachable and there is no direct route either — trying the proxy anyway"
            fi
        else
            log "no proxy configured — using direct egress"
        fi
        ;;
    *)
        PROXY="$EXPLICIT_PROXY"
        log "using explicitly configured proxy"
        ;;
esac

install -m 0755 -d /etc/apt/apt.conf.d
if [ -n "$PROXY" ]; then
    # Redact any embedded password before logging.
    SAFE_PROXY="$(printf '%s' "$PROXY" | sed -E 's#(://[^:/@]+):[^@]*@#\1:***@#')"
    log "HTTP(S) proxy: ${SAFE_PROXY} (no_proxy=${NO_PROXY_VAL})"

    cat > /etc/apt/apt.conf.d/00dockwin-proxy <<APT
Acquire::http::Proxy "${PROXY}";
Acquire::https::Proxy "${PROXY}";
APT

    # curl (Docker GPG key fetch, below) honors these; export for this script run.
    export http_proxy="$PROXY" https_proxy="$PROXY" no_proxy="$NO_PROXY_VAL"
    export HTTP_PROXY="$PROXY" HTTPS_PROXY="$PROXY" NO_PROXY="$NO_PROXY_VAL"
else
    # Direct mode: pin apt to DIRECT so it ignores any inherited proxy env (now and
    # for future `dockwin update` runs), and drop the proxy env for this run's
    # apt/curl children.
    cat > /etc/apt/apt.conf.d/00dockwin-proxy <<'APT'
Acquire::http::Proxy "DIRECT";
Acquire::https::Proxy "DIRECT";
APT
    unset http_proxy https_proxy no_proxy HTTP_PROXY HTTPS_PROXY NO_PROXY
fi

# ---------------------------------------------------------------------------
# 1. Base packages + Docker apt repo (pinned).
# ---------------------------------------------------------------------------
log "updating apt and installing prerequisites"
apt_update
apt-get install -y --no-install-recommends \
    ca-certificates curl gnupg lsb-release iptables socat uidmap

install -m 0755 -d /etc/apt/keyrings
if [ ! -s /etc/apt/keyrings/docker.asc ]; then
    log "fetching Docker GPG key"
    curl -fsSL "https://download.docker.com/linux/${DISTRO_ID}/gpg" \
        -o /etc/apt/keyrings/docker.asc
    chmod a+r /etc/apt/keyrings/docker.asc
fi

ARCH="$(dpkg --print-architecture)"
REPO_LINE="deb [arch=${ARCH} signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/${DISTRO_ID} ${CODENAME} stable"
echo "$REPO_LINE" > /etc/apt/sources.list.d/docker.list
log "configured pinned Docker repo: ${REPO_LINE}"

apt_update

# Pin to a known-good Docker version line for reproducible provisioning.
# Empty -> latest in the stable repo. Override via DOCKWIN_DOCKER_VERSION
# (e.g. "5:27.*" matching `apt-cache madison docker-ce`).
DOCKER_VERSION="${DOCKWIN_DOCKER_VERSION:-}"
# --no-install-recommends keeps the install lean (skips optional extras like
# pigz/slirp4netns/rootless that a rootful engine box doesn't need); iptables and
# the buildx/compose plugins we want are pulled in explicitly.
if [ -n "$DOCKER_VERSION" ]; then
    log "installing Docker Engine pinned to '${DOCKER_VERSION}'"
    apt-get install -y --no-install-recommends \
        "docker-ce=${DOCKER_VERSION}" \
        "docker-ce-cli=${DOCKER_VERSION}" \
        containerd.io docker-buildx-plugin docker-compose-plugin
else
    log "installing latest stable Docker Engine"
    apt-get install -y --no-install-recommends \
        docker-ce docker-ce-cli containerd.io \
        docker-buildx-plugin docker-compose-plugin
fi

# ---------------------------------------------------------------------------
# 2. iptables-legacy (nftables on newer Ubuntu silently breaks the docker
#    bridge even when dockerd starts cleanly -- documented WSL2 gotcha).
# ---------------------------------------------------------------------------
if update-alternatives --list iptables >/dev/null 2>&1; then
    if update-alternatives --list iptables | grep -q legacy; then
        log "switching iptables/ip6tables to legacy backend"
        update-alternatives --set iptables /usr/sbin/iptables-legacy || \
            warn "could not set iptables-legacy"
        update-alternatives --set ip6tables /usr/sbin/ip6tables-legacy || \
            warn "could not set ip6tables-legacy"
    else
        warn "iptables-legacy alternative not present; leaving default backend"
    fi
else
    warn "update-alternatives has no iptables entry; skipping legacy switch"
fi

# ---------------------------------------------------------------------------
# 3. daemon.json: cgroupfs cgroup driver + unix socket only.
#    cgroup v2 is the WSL2 default; without systemd as PID 1 there is no systemd
#    cgroup manager to defer to, so use the cgroupfs driver (dockerd manages the
#    cgroups itself), which works on WSL2 cgroup v2.
# ---------------------------------------------------------------------------
log "writing /etc/docker/daemon.json"
install -m 0755 -d /etc/docker

HOSTS_JSON='"unix:///var/run/docker.sock"'
if [ "$ENABLE_TCP" = "1" ]; then
    warn "DOCKWIN_ENABLE_TCP=1 -> ALSO binding tcp://127.0.0.1:2375 (INSECURE fallback, loopback only)"
    HOSTS_JSON='"unix:///var/run/docker.sock", "tcp://127.0.0.1:2375"'
fi

# Runtime egress (image pulls) goes through the same proxy when one is set. Modern
# dockerd reads proxy settings from daemon.json's "proxies" key (no systemd here,
# so no systemd drop-in). Emitted as its own line so an empty value leaves valid
# JSON (a blank line) rather than a dangling key.
PROXIES_JSON=""
if [ -n "$PROXY" ]; then
    PROXIES_JSON="  \"proxies\": { \"http-proxy\": \"${PROXY}\", \"https-proxy\": \"${PROXY}\", \"no-proxy\": \"${NO_PROXY_VAL}\" },"
fi

cat > /etc/docker/daemon.json <<JSON
{
${PROXIES_JSON}
  "hosts": [ ${HOSTS_JSON} ],
  "exec-opts": ["native.cgroupdriver=cgroupfs"],
  "iptables": true,
  "log-driver": "json-file",
  "log-opts": { "max-size": "10m", "max-file": "3" },
  "features": { "buildkit": true },
  "default-address-pools": [
    { "base": "172.30.0.0/16", "size": 24 }
  ]
}
JSON

# ---------------------------------------------------------------------------
# 4. Start dockerd via the non-systemd supervisor (like Docker Desktop). There
#    is no systemd here, so autostart/Restart-on-failure come from
#    /usr/local/sbin/dockwin-supervise.sh instead: on every distro start WSL runs
#    it from wsl.conf's [boot] command=, and it keeps dockerd alive in a detached
#    restart loop. It is placed by ops.rs before this script runs; invoke it now
#    (it self-detaches and is idempotent) so dockerd comes up during provisioning.
# ---------------------------------------------------------------------------
SUPERVISOR=/usr/local/sbin/dockwin-supervise.sh
[ -x "$SUPERVISOR" ] || die "supervisor $SUPERVISOR missing or not executable (should be placed by the installer)"
log "starting dockerd via $SUPERVISOR (non-systemd supervisor)"
"$SUPERVISOR"

# ---------------------------------------------------------------------------
# 5. Verify dockerd is actually reachable on its unix socket.
# ---------------------------------------------------------------------------
log "waiting for dockerd to become reachable on /var/run/docker.sock"
ok=0
for i in $(seq 1 30); do
    if docker version >/dev/null 2>&1; then
        ok=1
        break
    fi
    sleep 1
done

[ "$ok" -eq 1 ] || die "dockerd did not become reachable; check '/var/log/dockwin-dockerd.log' inside the distro"

log "dockerd is up:"
docker version --format '  server: {{.Server.Version}} (api {{.Server.APIVersion}})' || true

# ---------------------------------------------------------------------------
# 6. Optional bridge-connectivity smoke test (iptables sanity).
#    Best-effort: don't fail provisioning if the base image lacks the image
#    cache / network egress at provision time.
# ---------------------------------------------------------------------------
# The bridge test pulls hello-world from the network and runs it — a purely
# diagnostic smoke test (it never gated provisioning, only warned). It is the
# slow tail of provisioning, so it is OPT-IN: set DOCKWIN_RUN_NETTEST=1 to run
# it. dockerd and the network config are identical either way.
if [ "${DOCKWIN_RUN_NETTEST:-0}" = "1" ]; then
    log "running container bridge connectivity test (DOCKWIN_RUN_NETTEST=1)"
    if docker run --rm hello-world >/dev/null 2>&1; then
        log "container bridge networking OK"
    else
        warn "bridge test failed (no egress or image pull blocked); verify later with: docker run --rm hello-world"
    fi
else
    log "skipping bridge test (set DOCKWIN_RUN_NETTEST=1 to run it); verify anytime with: docker run --rm hello-world"
fi

log "provisioning complete."
