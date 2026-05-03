# Scope, MVP, and Roadmap

## MVP Scope

Agent1 MVP focuses on proving the local agent runtime.

### Included

- Rust workspace
- CLI
- Tauri desktop shell
- SQLite persistence
- Ollama model adapter
- OpenAI-compatible local adapter
- Agent config
- Single-agent runtime
- Basic multi-agent handoff
- Native tool system
- File read/write tools
- Shell tool with approval
- Git status/diff tools
- Event log
- Basic memory
- MCP stdio client
- Agent cards
- Agent graph UI

### Excluded

- Cloud deployment platform
- Hosted accounts
- Payments
- Mobile app
- Browser automation
- Enterprise SSO
- Full remote A2A compliance
- Marketplace

## MVP Definition of Done

- User can install Agent1.
- User can connect local model.
- User can create agent.
- User can run task.
- Agent can use approved tool.
- Agent trace is saved.
- UI displays session and events.
- MCP stdio server can be connected.
- Host agent can call worker agent.

## Roadmap

### Phase 0: Foundations

- Repo setup
- Rust workspace
- Core schemas
- Config loader
- Logging
- SQLite migrations

### Phase 1: Model Layer

- Ollama adapter
- OpenAI-compatible local adapter
- Model list
- Chat request/response

### Phase 2: Single Agent Runtime

- Agent config
- Prompt builder
- Session state
- Run loop
- CLI execution

### Phase 3: Tools

- Tool trait
- Tool registry
- File read
- File write
- Shell command
- Permission guard

### Phase 4: Memory

- Session memory
- Long-term memory
- Keyword search
- Memory viewer

### Phase 5: MCP

- MCP stdio client
- Tool discovery
- Tool invocation
- MCP tool permissions

### Phase 6: Multi-Agent

- Agent cards
- Agent skills
- Agent call tool
- Supervisor/planner/worker/critic flow

### Phase 7: Desktop UI

- Dashboard
- Agent builder
- Chat runner
- Event feed
- Agent graph
- MCP manager

### Phase 8: Local A2A Server

- Agent card endpoint
- Task endpoint
- Status endpoint
- Event stream endpoint

### Phase 9: Release

- Installers
- Docker compose
- Docs
- Examples
- Security guide
