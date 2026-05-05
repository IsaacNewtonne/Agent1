CREATE TABLE IF NOT EXISTS orchestration_sessions (
    id TEXT PRIMARY KEY,
    objective TEXT NOT NULL,
    plan_id TEXT,
    status TEXT NOT NULL DEFAULT 'received',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS execution_plans (
    id TEXT PRIMARY KEY,
    orchestration_id TEXT NOT NULL,
    objective TEXT NOT NULL,
    raw_goal TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY(orchestration_id) REFERENCES orchestration_sessions(id)
);

CREATE TABLE IF NOT EXISTS execution_steps (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    description TEXT NOT NULL,
    step_order INTEGER NOT NULL,
    assigned_agent_id TEXT,
    assigned_role TEXT,
    dependencies TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    output TEXT,
    review_notes TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    FOREIGN KEY(plan_id) REFERENCES execution_plans(id)
);

CREATE TABLE IF NOT EXISTS escalation_queue (
    id TEXT PRIMARY KEY,
    orchestration_id TEXT NOT NULL,
    step_id TEXT,
    escalation_type TEXT NOT NULL,
    description TEXT NOT NULL,
    payload TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    response TEXT,
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY(orchestration_id) REFERENCES orchestration_sessions(id),
    FOREIGN KEY(step_id) REFERENCES execution_steps(id)
);

CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_status ON orchestration_sessions(status);
CREATE INDEX IF NOT EXISTS idx_orchestration_sessions_created_at ON orchestration_sessions(created_at);
CREATE INDEX IF NOT EXISTS idx_execution_plans_orchestration_id ON execution_plans(orchestration_id);
CREATE INDEX IF NOT EXISTS idx_execution_steps_plan_id ON execution_steps(plan_id);
CREATE INDEX IF NOT EXISTS idx_execution_steps_status ON execution_steps(status);
CREATE INDEX IF NOT EXISTS idx_escalation_queue_status ON escalation_queue(status);
CREATE INDEX IF NOT EXISTS idx_escalation_queue_orchestration_id ON escalation_queue(orchestration_id);