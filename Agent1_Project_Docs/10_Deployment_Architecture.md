# Deployment Architecture

## Deployment Modes

## Desktop Mode

Default mode.

```text
Tauri Desktop UI
↓
Embedded/local Agent1 server
↓
SQLite + local files
↓
Ollama/MCP/local tools
```

Best for:

- Individual users
- Developers
- Local workflows

## CLI Mode

```text
agent1 CLI
↓
Agent1 runtime
↓
SQLite + local tools
```

Best for:

- Developers
- Automation
- Testing

## Headless Server Mode

```text
Local/private server
↓
Agent1 HTTP API
↓
SQLite
↓
Local model provider
```

Best for:

- Home server
- Private team server
- VPS with GPU

Server mode must require authentication if accessible beyond localhost.

## Docker Mode

Optional after MVP.

```text
docker compose
├── agent1
├── ollama
└── volume: agent1-data
```

## Network Binding Rules

| Mode | Default Bind |
|---|---|
| Desktop | 127.0.0.1 |
| CLI | none |
| Server | 127.0.0.1 |
| Public server | explicit config only |

## Data Volumes

Mount:

```text
/var/lib/agent1
```

or user-selected path.

## Update Strategy

- Desktop installers update app binary.
- Database migrations run on startup.
- Config migrations run with backup.
- User data is never deleted during update.
