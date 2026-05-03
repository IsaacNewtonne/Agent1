# Database Schema and Migrations

## Database

Agent1 uses SQLite.

## Migration Strategy

- Use numbered SQL migrations.
- Never edit an applied migration.
- Add forward-only migrations.
- Include indexes with initial schema.
- Store app schema version.

## Tables

## agents

```sql
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    role TEXT,
    system_prompt TEXT NOT NULL,
    model_config_json TEXT NOT NULL,
    memory_config_json TEXT NOT NULL,
    permissions_json TEXT NOT NULL,
    max_iterations INTEGER NOT NULL DEFAULT 12,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## sessions

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    title TEXT,
    root_agent_id TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## messages

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    from_agent_id TEXT,
    to_agent_id TEXT,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);
```

## tool_calls

```sql
CREATE TABLE tool_calls (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    input_json TEXT NOT NULL,
    output_json TEXT,
    status TEXT NOT NULL,
    error TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);
```

## events

```sql
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

## memories

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    agent_id TEXT,
    content TEXT NOT NULL,
    tags_json TEXT,
    embedding_json TEXT,
    importance INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## agent_cards

```sql
CREATE TABLE agent_cards (
    agent_id TEXT PRIMARY KEY,
    card_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## mcp_servers

```sql
CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    transport TEXT NOT NULL,
    command TEXT,
    args_json TEXT,
    env_json TEXT,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## artifacts

```sql
CREATE TABLE artifacts (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT,
    path TEXT NOT NULL,
    mime_type TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL
);
```

## approvals

```sql
CREATE TABLE approvals (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    request_json TEXT NOT NULL,
    decision TEXT,
    decided_at TEXT,
    created_at TEXT NOT NULL
);
```

## Indexes

```sql
CREATE INDEX idx_messages_session_id ON messages(session_id);
CREATE INDEX idx_events_session_id ON events(session_id);
CREATE INDEX idx_events_created_at ON events(created_at);
CREATE INDEX idx_tool_calls_session_id ON tool_calls(session_id);
CREATE INDEX idx_memories_agent_id ON memories(agent_id);
CREATE INDEX idx_artifacts_session_id ON artifacts(session_id);
```
