#!/bin/bash
# dockwin-supervise.sh — non-systemd dockerd supervisor.
#
# Started by /etc/wsl.conf [boot] command= on every distro start, as root, early.
# This REPLACES systemd's "systemctl enable docker" + Restart=on-failure: it keeps
# dockerd alive in a detached restart loop. MUST return quickly — WSL runs the boot
# command inline — so the real work is handed to a detached (setsid) supervisor and
# this script exits 0. Idempotent: a second boot-command invocation is a no-op while
# a supervisor is alive. /run/dockwin-supervise.pid holds the SUPERVISOR LOOP pid so
# ops::stop can stop respawning before it SIGTERMs dockerd.
#
# CONCURRENCY: ops::start(), ensure_dockerd(), update_engine and the wsl.conf [boot]
# command all invoke this script, sometimes milliseconds apart (start() boots the
# distro — running the boot command — then calls this again). The idempotency guard
# below must not race: because the detached supervisor writes the pidfile only AFTER
# it has been backgrounded, a naive guard lets two near-simultaneous invocations both
# see "no live pidfile" and each spawn a loop, orphaning one (ops::stop then can only
# kill the pid last written) and leaving two dockerds colliding on docker.pid/.sock.
# We therefore serialize the whole guard+spawn under an flock and DO NOT release it
# until the detached loop has published its pid, so any invocation blocked on the lock
# observes a live pidfile and backs off instead of spawning a second loop.
set -e

# A full PATH is REQUIRED: the [boot] command= context is minimal. The supervisor loop
# needs it so dockerd finds /usr/bin/docker-proxy (else "userland-proxy-path is not
# set"); the parent needs it here so flock/setsid/cat/sleep/seq resolve too.
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

PIDFILE=/run/dockwin-supervise.pid
LOCKFILE=/run/dockwin-supervise.lock
LOG=/var/log/dockwin-dockerd.log

# Serialize concurrent invocations. Holding fd 9 open across the guard, the spawn, and
# the wait-for-pid below makes the whole critical section atomic w.r.t. other callers.
exec 9>"$LOCKFILE"
flock 9

# Idempotent: a live supervisor loop means there is nothing to do.
if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE" 2>/dev/null)" 2>/dev/null; then
    exit 0
fi
rm -f "$PIDFILE"

# dockerd manages its OWN containerd child here (no --containerd flag), so there is
# no separate service to order against. Close the lock fd (9>&-) in the detached child
# so the long-lived supervisor loop does NOT keep the flock held forever (which would
# deadlock every future invocation).
setsid bash -c '
    export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
    echo $$ > '"$PIDFILE"'
    while true; do
        /usr/bin/dockerd >>'"$LOG"' 2>&1
        echo "[supervise] dockerd exited ($?), restarting in 2s" >>'"$LOG"'
        sleep 2
    done
' </dev/null >/dev/null 2>&1 9>&- &

# Do not release the flock (by exiting) until the detached loop has published its live
# pid, so a concurrent invocation waiting on the lock sees a live pidfile and backs off
# rather than spawning a second, orphaned supervisor loop. Bounded (~5s) so the boot
# command can never hang; the child normally writes its pid within a few milliseconds.
for _ in $(seq 1 100); do
    if [ -s "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE" 2>/dev/null)" 2>/dev/null; then
        break
    fi
    sleep 0.05
done

exit 0
