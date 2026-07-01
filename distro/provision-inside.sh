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
# 1. Base packages + Docker apt repo (pinned).
# ---------------------------------------------------------------------------
log "updating apt and installing prerequisites"
apt-get update -y
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

apt-get update -y

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

cat > /etc/docker/daemon.json <<JSON
{
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
