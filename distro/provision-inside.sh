#!/usr/bin/env bash
#
# provision-inside.sh - run INSIDE the dedicated "dockwin" WSL2 distro as root.
#
# Installs Docker Engine from the PINNED official Docker apt repo (not the
# unpinned get.docker.com convenience script), configures the documented WSL2
# gotchas (iptables-legacy, systemd cgroup driver, cgroup v2), enables dockerd
# autostart via systemd, and verifies the daemon comes up on its UNIX socket.
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
# 3. daemon.json: systemd cgroup driver + unix socket only.
#    cgroup v2 is the WSL2 default; pair it with the systemd cgroup driver.
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
  "exec-opts": ["native.cgroupdriver=systemd"],
  "iptables": true,
  "log-driver": "json-file",
  "log-opts": { "max-size": "10m", "max-file": "3" },
  "features": { "buildkit": true },
  "default-address-pools": [
    { "base": "172.30.0.0/16", "size": 24 }
  ]
}
JSON

# When daemon.json sets "hosts", the systemd unit's own `-H fd://` collides
# with it ("unable to configure the Docker daemon ... hosts ... both"). Drop
# the unit-level -H via a drop-in so daemon.json is the single source of truth.
log "writing docker.service drop-in (let daemon.json own -H)"
install -m 0755 -d /etc/systemd/system/docker.service.d
cat > /etc/systemd/system/docker.service.d/10-dockwin-hosts.conf <<'UNIT'
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd --containerd=/run/containerd/containerd.sock
UNIT

# ---------------------------------------------------------------------------
# 4. Enable + (re)start dockerd. Use systemd when it is PID 1, otherwise fall
#    back to the sysv service (the non-systemd autostart path).
# ---------------------------------------------------------------------------
if [ -d /run/systemd/system ]; then
    log "systemd detected: enabling and starting docker via systemctl"
    systemctl daemon-reload
    systemctl enable docker.service containerd.service >/dev/null 2>&1 || \
        warn "systemctl enable reported a problem"
    # NOTE: we deliberately do NOT try to disable docker-ce's sysv autostart
    # here. On Debian `update-rc.d` and `systemctl enable` are kept in sync by
    # systemd-sysv-install (disabling sysv also disables the unit, and vice
    # versa), so you can't have systemd-on + sysv-off via these tools. It isn't
    # needed anyway: a native docker.service makes systemd ignore the sysv
    # runlevel links, so the ONLY path that can launch the pidfile-stealing bare
    # `dockerd -p /var/run/docker.pid` is wsl.conf's `[boot] command=`, which
    # waits for systemd before deciding so it can't race it (see distro/wsl.conf).
    systemctl restart docker.service
else
    warn "systemd is NOT PID 1; using sysv 'service docker start' fallback"
    update-rc.d docker defaults >/dev/null 2>&1 || true
    service docker restart || service docker start
fi

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

[ "$ok" -eq 1 ] || die "dockerd did not become reachable; check 'journalctl -u docker' inside the distro"

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
