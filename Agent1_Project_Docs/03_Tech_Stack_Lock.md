# Tech Stack Lock

## Backend

| Area | Locked Choice |
|---|---|
| Language | Rust |
| Async runtime | Tokio |
| HTTP server | Axum |
| HTTP client | Reqwest |
| Serialization | Serde |
| Error handling | thiserror + anyhow |
| Logging/tracing | tracing + tracing-subscriber |
| Database | SQLite |
| Database crate | SQLx |
| CLI | Clap |
| Config | TOML + JSON |
| IDs | UUID v7 or ULID |
| Time | time or chrono |
| JSON Schema | schemars |
| Validation | validator or custom validation |

## Frontend

| Area | Locked Choice |
|---|---|
| Desktop shell | Tauri |
| UI framework | React or Svelte |
| Graph UI | React Flow or Svelte Flow |
| Styling | Tailwind CSS |
| State management | Lightweight store |
| Build tool | Vite |

Default recommendation: **Tauri + React + Tailwind + React Flow**.

## Model Backends

| Backend | Priority |
|---|---|
| Ollama | P0 |
| OpenAI-compatible local endpoint | P0 |
| llama.cpp server | P1 |
| vLLM | P1 |
| Native Rust inference | P2 |

## Storage

| Data | Storage |
|---|---|
| Config | TOML/JSON files |
| Sessions | SQLite |
| Messages | SQLite |
| Events | SQLite |
| Tool calls | SQLite |
| Memory | SQLite |
| Artifacts | Filesystem + SQLite index |
| Logs | Local log files |

## Protocols

| Protocol | Use |
|---|---|
| HTTP | Local server API |
| WebSocket | UI event stream |
| stdio | MCP local server transport |
| JSON-RPC | MCP/A2A-style messages where useful |

## Explicitly Not Used in MVP

- Kubernetes
- Cloud database
- Hosted auth
- Paid AI APIs
- Electron
- Python runtime requirement
- Browser automation
