"""DDL schema strings for the event-sourced store tables."""

# ── SQLite schemas ────────────────────────────────────────────────────────

SQLITE_EVENTS = """\
CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id    TEXT    NOT NULL UNIQUE,
    timestamp   TEXT    NOT NULL,
    workflow_id TEXT    NOT NULL,
    event_type  TEXT    NOT NULL,
    payload     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_workflow  ON events(workflow_id);
CREATE INDEX IF NOT EXISTS idx_events_type      ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
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
    created_at          TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dispatch_status  ON dispatch_projection(status);
CREATE INDEX IF NOT EXISTS idx_dispatch_lane    ON dispatch_projection(lane);
CREATE INDEX IF NOT EXISTS idx_dispatch_created ON dispatch_projection(created_at);
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

SQLITE_ALL = SQLITE_EVENTS + SQLITE_DISPATCH_PROJECTION + SQLITE_STEP_PROJECTION

# ── Postgres schemas ──────────────────────────────────────────────────────

POSTGRES_EVENTS = """\
CREATE TABLE IF NOT EXISTS events (
    id          BIGSERIAL PRIMARY KEY,
    event_id    TEXT      NOT NULL UNIQUE,
    timestamp   TEXT      NOT NULL,
    workflow_id TEXT      NOT NULL,
    event_type  TEXT      NOT NULL,
    payload     JSONB     NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_workflow  ON events(workflow_id);
CREATE INDEX IF NOT EXISTS idx_events_type      ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
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
    created_at          TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dispatch_status  ON dispatch_projection(status);
CREATE INDEX IF NOT EXISTS idx_dispatch_lane    ON dispatch_projection(lane);
CREATE INDEX IF NOT EXISTS idx_dispatch_created ON dispatch_projection(created_at);
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

POSTGRES_ALL = POSTGRES_EVENTS + POSTGRES_DISPATCH_PROJECTION + POSTGRES_STEP_PROJECTION
