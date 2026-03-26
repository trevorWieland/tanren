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

# Export secrets from mounted secrets.env into the daemon environment.
# These are consumed by required_secrets resolution during provision
# (e.g. CLAUDE_CODE_OAUTH_TOKEN, OPENCODE_ZAI_API_KEY).
SECRETS_FILE="/config/secrets.env"
if [ -f "$SECRETS_FILE" ]; then
    set -a
    # shellcheck disable=SC1090
    . "$SECRETS_FILE"
    set +a
fi

# Grant app user access to Docker socket (DooD).
# Resolve the group that owns the socket GID (or create one), then add app.
DOCKER_SOCK="/var/run/docker.sock"
if [ -S "$DOCKER_SOCK" ]; then
    SOCK_GID=$(stat -c '%g' "$DOCKER_SOCK")
    # Look up existing group for this GID; create "docker" if none exists.
    SOCK_GROUP=$(getent group "$SOCK_GID" 2>/dev/null | cut -d: -f1 || true)
    if [ -z "$SOCK_GROUP" ]; then
        addgroup --gid "$SOCK_GID" docker >/dev/null 2>&1
        SOCK_GROUP="docker"
    fi
    adduser app "$SOCK_GROUP" >/dev/null 2>&1 || true
fi

# Drop privileges and run the daemon
exec gosu app tanren-daemon "$@"
