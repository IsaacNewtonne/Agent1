-- Collaboration workspace tables

CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT,
    collab_mode     TEXT NOT NULL DEFAULT 'automatic',
    agent_ids_json  TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS blackboard (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    key             TEXT NOT NULL,
    value_json      TEXT NOT NULL DEFAULT '{}',
    author_agent_id TEXT NOT NULL,
    author_type     TEXT NOT NULL DEFAULT 'local',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_blackboard_project_key
    ON blackboard(project_id, key);

CREATE TABLE IF NOT EXISTS external_agents (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    endpoint        TEXT,
    invite_token    TEXT NOT NULL,
    capabilities_json TEXT NOT NULL DEFAULT '[]',
    permissions_json  TEXT NOT NULL DEFAULT '{}',
    status          TEXT NOT NULL DEFAULT 'invited',
    last_heartbeat  TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_external_agents_project
    ON external_agents(project_id);

CREATE INDEX IF NOT EXISTS idx_external_agents_token
    ON external_agents(invite_token);

CREATE TABLE IF NOT EXISTS invite_tokens (
    token           TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    project_name    TEXT NOT NULL,
    permissions_json TEXT NOT NULL DEFAULT '{}',
    created_by      TEXT NOT NULL,
    gateway_url     TEXT,
    expires_at      TEXT,
    used_by         TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collab_tasks (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    description         TEXT NOT NULL,
    assigned_agent_id   TEXT,
    assigned_agent_type TEXT,
    status              TEXT NOT NULL DEFAULT 'queued',
    output              TEXT,
    requires_approval   INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    completed_at        TEXT
);

CREATE INDEX IF NOT EXISTS idx_collab_tasks_project
    ON collab_tasks(project_id);

CREATE TABLE IF NOT EXISTS collab_events (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    agent_id        TEXT,
    payload_json    TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_collab_events_project
    ON collab_events(project_id, created_at);
