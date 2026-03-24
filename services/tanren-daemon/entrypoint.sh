#!/bin/sh
set -e

# Copy mounted SSH keys with correct ownership for the app user.
# Host-mounted keys have host UID ownership; the app user (10001) can't read them.
SSH_SRC="/config/ssh"
SSH_DST="/app/.ssh"
if [ -d "$SSH_SRC" ] && [ "$(ls -A "$SSH_SRC" 2>/dev/null)" ]; then
    mkdir -p "$SSH_DST"
    cp "$SSH_SRC"/* "$SSH_DST"/ 2>/dev/null || true
    chmod 700 "$SSH_DST"
    chmod 600 "$SSH_DST"/* 2>/dev/null || true
    chown -R app:app "$SSH_DST"
fi

# Drop privileges and run the daemon
exec gosu app tanren-daemon "$@"
