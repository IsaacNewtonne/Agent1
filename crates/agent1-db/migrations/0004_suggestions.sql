-- Suggestions table for proactive recommendation system

CREATE TABLE IF NOT EXISTS suggestions (
    id                  TEXT PRIMARY KEY,
    suggestion_type     TEXT NOT NULL,
    content             TEXT NOT NULL,
    trigger_context     TEXT NOT NULL DEFAULT '',
    related_memory_id   TEXT,
    status              TEXT NOT NULL DEFAULT 'pending',
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    accepted_at         TEXT,
    dismissed_at        TEXT
);

CREATE INDEX IF NOT EXISTS idx_suggestions_status
    ON suggestions(status);

CREATE INDEX IF NOT EXISTS idx_suggestions_type
    ON suggestions(suggestion_type);

CREATE INDEX IF NOT EXISTS idx_suggestions_created
    ON suggestions(created_at);