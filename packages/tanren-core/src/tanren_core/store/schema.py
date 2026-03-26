"""DDL schema strings for the event-sourced store tables."""

# ── SQLite schemas ────────────────────────────────────────────────────────

SQLITE_EVENTS = """\
CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id    TEXT    NOT NULL UNIQUE,
    timestamp   TEXT    NOT NULL,
    entity_id   TEXT    NOT NULL,
    entity_type TEXT    NOT NULL DEFAULT 'dispatch',
    event_type  TEXT    NOT NULL,
    payload     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_entity      ON events(entity_id);
CREATE INDEX IF NOT EXISTS idx_events_entity_type ON events(entity_type);
CREATE INDEX IF NOT EXISTS idx_events_type        ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp   ON events(timestamp);
"""

SQLITE_DISPATCH_PROJECTION = """\
CREATE TABLE IF NOT EXISTS dispatch_projection (
    dispatch_id         TEXT    PRIMARY KEY,
    mode                TEXT    NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'pending',
    outcome             TEXT,
    lane                TEXT    NOT NULL,
    preserve_on_failure INTEGER NOT NULL DEFAULT 0,
    dispatch_json       TEXT    NOT NULL,
    user_id             TEXT    NOT NULL DEFAULT '',
    created_at          TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dispatch_status  ON dispatch_projection(status);
CREATE INDEX IF NOT EXISTS idx_dispatch_lane    ON dispatch_projection(lane);
CREATE INDEX IF NOT EXISTS idx_dispatch_created ON dispatch_projection(created_at);
CREATE INDEX IF NOT EXISTS idx_dispatch_user    ON dispatch_projection(user_id);
"""

SQLITE_STEP_PROJECTION = """\
CREATE TABLE IF NOT EXISTS step_projection (
    step_id        TEXT    PRIMARY KEY,
    dispatch_id    TEXT    NOT NULL,
    step_type      TEXT    NOT NULL,
    step_sequence  INTEGER NOT NULL,
    lane           TEXT,
    status         TEXT    NOT NULL DEFAULT 'pending',
    worker_id      TEXT,
    payload_json   TEXT    NOT NULL,
    result_json    TEXT,
    error          TEXT,
    retry_count    INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL,
    updated_at     TEXT    NOT NULL,
    FOREIGN KEY (dispatch_id) REFERENCES dispatch_projection(dispatch_id)
);
CREATE INDEX IF NOT EXISTS idx_step_dispatch    ON step_projection(dispatch_id);
CREATE INDEX IF NOT EXISTS idx_step_status      ON step_projection(status);
CREATE INDEX IF NOT EXISTS idx_step_lane_status ON step_projection(lane, status);
"""

SQLITE_VM_ASSIGNMENTS = """\
CREATE TABLE IF NOT EXISTS vm_assignments (
    vm_id       TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    project     TEXT NOT NULL,
    spec        TEXT NOT NULL,
    host        TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    released_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_vm_active
    ON vm_assignments(released_at) WHERE released_at IS NULL;
"""

SQLITE_USER_PROJECTION = """\
CREATE TABLE IF NOT EXISTS user_projection (
    user_id    TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    email      TEXT,
    role       TEXT NOT NULL DEFAULT 'member',
    is_active  INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"""

SQLITE_API_KEY_PROJECTION = """\
CREATE TABLE IF NOT EXISTS api_key_projection (
    key_id            TEXT PRIMARY KEY,
    user_id           TEXT NOT NULL,
    name              TEXT NOT NULL,
    key_prefix        TEXT NOT NULL,
    key_hash          TEXT NOT NULL UNIQUE,
    scopes            TEXT NOT NULL,
    resource_limits   TEXT,
    created_at        TEXT NOT NULL,
    expires_at        TEXT,
    revoked_at        TEXT,
    grace_replaced_by TEXT,
    FOREIGN KEY (user_id) REFERENCES user_projection(user_id)
);
CREATE INDEX IF NOT EXISTS idx_key_hash   ON api_key_projection(key_hash);
CREATE INDEX IF NOT EXISTS idx_key_user   ON api_key_projection(user_id);
CREATE INDEX IF NOT EXISTS idx_key_prefix ON api_key_projection(key_prefix);
"""

SQLITE_ALL = (
    SQLITE_EVENTS
    + SQLITE_DISPATCH_PROJECTION
    + SQLITE_STEP_PROJECTION
    + SQLITE_VM_ASSIGNMENTS
    + SQLITE_USER_PROJECTION
    + SQLITE_API_KEY_PROJECTION
)

# ── Postgres schemas ──────────────────────────────────────────────────────

POSTGRES_EVENTS = """\
CREATE TABLE IF NOT EXISTS events (
    id          BIGSERIAL PRIMARY KEY,
    event_id    TEXT      NOT NULL UNIQUE,
    timestamp   TEXT      NOT NULL,
    entity_id   TEXT      NOT NULL,
    entity_type TEXT      NOT NULL DEFAULT 'dispatch',
    event_type  TEXT      NOT NULL,
    payload     JSONB     NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_entity      ON events(entity_id);
CREATE INDEX IF NOT EXISTS idx_events_entity_type ON events(entity_type);
CREATE INDEX IF NOT EXISTS idx_events_type        ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp   ON events(timestamp);
"""

POSTGRES_DISPATCH_PROJECTION = """\
CREATE TABLE IF NOT EXISTS dispatch_projection (
    dispatch_id         TEXT    PRIMARY KEY,
    mode                TEXT    NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'pending',
    outcome             TEXT,
    lane                TEXT    NOT NULL,
    preserve_on_failure BOOLEAN NOT NULL DEFAULT FALSE,
    dispatch_json       JSONB   NOT NULL,
    user_id             TEXT    NOT NULL DEFAULT '',
    created_at          TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dispatch_status  ON dispatch_projection(status);
CREATE INDEX IF NOT EXISTS idx_dispatch_lane    ON dispatch_projection(lane);
CREATE INDEX IF NOT EXISTS idx_dispatch_created ON dispatch_projection(created_at);
CREATE INDEX IF NOT EXISTS idx_dispatch_user    ON dispatch_projection(user_id);
"""

POSTGRES_STEP_PROJECTION = """\
CREATE TABLE IF NOT EXISTS step_projection (
    step_id        TEXT    PRIMARY KEY,
    dispatch_id    TEXT    NOT NULL REFERENCES dispatch_projection(dispatch_id),
    step_type      TEXT    NOT NULL,
    step_sequence  INTEGER NOT NULL,
    lane           TEXT,
    status         TEXT    NOT NULL DEFAULT 'pending',
    worker_id      TEXT,
    payload_json   JSONB   NOT NULL,
    result_json    JSONB,
    error          TEXT,
    retry_count    INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL,
    updated_at     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_step_dispatch    ON step_projection(dispatch_id);
CREATE INDEX IF NOT EXISTS idx_step_status      ON step_projection(status);
CREATE INDEX IF NOT EXISTS idx_step_lane_status ON step_projection(lane, status);
"""

POSTGRES_VM_ASSIGNMENTS = """\
CREATE TABLE IF NOT EXISTS vm_assignments (
    vm_id       TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    project     TEXT NOT NULL,
    spec        TEXT NOT NULL,
    host        TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    released_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_vm_active
    ON vm_assignments(released_at) WHERE released_at IS NULL;
"""

POSTGRES_USER_PROJECTION = """\
CREATE TABLE IF NOT EXISTS user_projection (
    user_id    TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    email      TEXT,
    role       TEXT NOT NULL DEFAULT 'member',
    is_active  BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"""

POSTGRES_API_KEY_PROJECTION = """\
CREATE TABLE IF NOT EXISTS api_key_projection (
    key_id            TEXT PRIMARY KEY,
    user_id           TEXT NOT NULL REFERENCES user_projection(user_id),
    name              TEXT NOT NULL,
    key_prefix        TEXT NOT NULL,
    key_hash          TEXT NOT NULL UNIQUE,
    scopes            JSONB NOT NULL,
    resource_limits   JSONB,
    created_at        TEXT NOT NULL,
    expires_at        TEXT,
    revoked_at        TEXT,
    grace_replaced_by TEXT
);
CREATE INDEX IF NOT EXISTS idx_key_hash   ON api_key_projection(key_hash);
CREATE INDEX IF NOT EXISTS idx_key_user   ON api_key_projection(user_id);
CREATE INDEX IF NOT EXISTS idx_key_prefix ON api_key_projection(key_prefix);
"""

POSTGRES_ALL = (
    POSTGRES_EVENTS
    + POSTGRES_DISPATCH_PROJECTION
    + POSTGRES_STEP_PROJECTION
    + POSTGRES_VM_ASSIGNMENTS
    + POSTGRES_USER_PROJECTION
    + POSTGRES_API_KEY_PROJECTION
)
