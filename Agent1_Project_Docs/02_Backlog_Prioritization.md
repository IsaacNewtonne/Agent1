# Backlog Prioritization

## Priority Definitions

- P0: Required for MVP
- P1: Required for strong beta
- P2: Important later
- P3: Nice to have

## P0 Backlog

| Item | Description |
|---|---|
| Rust workspace | Modular Cargo workspace |
| Config loader | TOML/JSON app and agent config |
| SQLite persistence | Sessions, messages, events, tools |
| Ollama adapter | Local model calls |
| CLI run command | Run agent from terminal |
| Agent config | Name, instructions, model, tools, permissions |
| Prompt builder | Build model prompt from agent and session |
| Agent loop | Model/tool/final response loop |
| Tool trait | Common interface for tools |
| File read tool | Approved workspace file reading |
| File write tool | Approved workspace file writing |
| Shell tool | Command execution with approval |
| Permission guard | Ask/allow/deny policy |
| Event log | Structured runtime events |
| Tauri shell | Desktop foundation |
| Session viewer | Chat and trace display |
| Agent builder | Create/edit agents |
| MCP stdio client | Connect local MCP servers |
| Agent card | Local agent metadata |
| Agent call tool | Basic agent-to-agent handoff |

## P1 Backlog

| Item | Description |
|---|---|
| Streaming responses | Provider streaming support |
| Memory viewer | Inspect/edit/delete memories |
| Git diff tool | Repo inspection |
| Cargo tools | cargo check/test commands |
| Agent graph | Live visual graph |
| Export session | Markdown/JSON export |
| Local A2A HTTP server | Expose agents locally |
| Tool allowlists | User-managed policy |
| Workspace sandbox | Safer file boundaries |

## P2 Backlog

| Item | Description |
|---|---|
| Vector memory | Local embedding search |
| Remote MCP transport | HTTP/SSE support |
| Plugin SDK | Easier third-party tools |
| Model benchmark panel | Compare local models |
| Workflow templates | Reusable agent teams |
| Patch application | Safer code edits |

## P3 Backlog

| Item | Description |
|---|---|
| Marketplace | Community agent library |
| Themes | UI customization |
| Mobile companion | Remote view/control |
| Multi-user server | Team mode |
