# Data Plan

## Data Principles

- Store locally by default.
- Use SQLite for structured runtime data.
- Store artifacts on filesystem with database references.
- Make user data exportable.
- Make user data deletable.
- Do not collect telemetry by default.

## Data Types

| Data Type | Storage |
|---|---|
| Agent configs | SQLite + optional TOML files |
| App config | TOML |
| Sessions | SQLite |
| Messages | SQLite |
| Events | SQLite |
| Tool calls | SQLite |
| MCP server configs | SQLite/TOML |
| Memories | SQLite |
| Artifacts | Filesystem + SQLite index |
| Logs | Local files |
| Approvals | SQLite |

## Data Directories

Recommended default:

```text
~/.agent1/
├── config.toml
├── agent1.db
├── agents/
├── artifacts/
├── logs/
├── mcp/
└── backups/
```

On Windows:

```text
%APPDATA%/Agent1/
```

## Export Formats

Agent1 should support:

- Session Markdown export
- Session JSON trace export
- Agent config TOML export
- Memory JSON export
- Artifact folder export

## Deletion

User must be able to delete:

- A session
- All sessions
- A memory item
- All memories
- An agent
- Tool call logs
- Artifacts

Deletion should not break database integrity.
