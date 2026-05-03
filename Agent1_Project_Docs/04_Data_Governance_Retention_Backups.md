# Data Governance, Retention, and Backups

## Governance Principles

- User owns all local data.
- No mandatory telemetry.
- No data leaves the machine unless the user configures remote endpoints.
- Logs and traces must be inspectable.
- Sensitive values must be redacted.

## Retention Defaults

| Data | Default Retention |
|---|---|
| Sessions | Keep until deleted |
| Messages | Keep until session deleted |
| Events | Keep until session deleted |
| Tool calls | Keep until session deleted |
| Artifacts | Keep until deleted |
| Logs | 30 days |
| Memory | Keep until deleted |
| Approvals | Keep until session deleted |

## User Controls

Users can:

- Delete all sessions.
- Delete one session.
- Delete all memories.
- Delete one memory.
- Delete logs.
- Delete artifacts.
- Export before deletion.

## Backups

Agent1 should support manual backup:

```text
agent1 backup create
```

Backup contents:

- SQLite database
- Agent configs
- App config
- Artifacts
- MCP configs

Backup format:

```text
agent1-backup-YYYYMMDD-HHMMSS.zip
```

## Restore

```text
agent1 backup restore ./agent1-backup.zip
```

Restore should:

- Validate backup structure.
- Stop active runs before restore.
- Create backup of current state before overwrite.

## Redaction

Never store unredacted:

- API keys
- Passwords
- Access tokens
- SSH keys
- Private keys

Use secret references instead of raw secrets.
