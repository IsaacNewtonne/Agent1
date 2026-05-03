# Release Plan and Versioning

## Versioning

Agent1 uses semantic versioning:

```text
MAJOR.MINOR.PATCH
```

Before `1.0.0`, breaking changes are allowed but should be documented.

## Release Stages

### v0.1.0: CLI Runtime Alpha

Includes:

- Ollama adapter
- Agent TOML config
- CLI run command
- SQLite sessions
- Basic file read tool
- Event log

### v0.2.0: Tooling Alpha

Includes:

- File write tool
- Shell tool with approval
- Git tools
- Permission policies
- Better error handling

### v0.3.0: MCP Alpha

Includes:

- MCP stdio client
- MCP tool discovery
- MCP tool invocation
- MCP tool logging

### v0.4.0: Multi-Agent Alpha

Includes:

- Agent cards
- Agent skills
- Agent call tool
- Planner/worker/critic examples

### v0.5.0: Desktop Alpha

Includes:

- Tauri app
- Dashboard
- Agent builder
- Session viewer
- Event feed
- Approval modal

### v0.8.0: Beta

Includes:

- Agent graph
- Memory viewer
- Export
- Installers
- Better docs

### v1.0.0: Stable Local MVP

Includes:

- Stable config schema
- Stable database migrations
- Stable native tool interface
- Stable local agent card
- Security review complete
- Cross-platform installers

## Release Requirements

Every release must include:

- Changelog entry
- Migration notes
- Test pass
- Security notes
- Example agent configs
- Build artifacts where possible
