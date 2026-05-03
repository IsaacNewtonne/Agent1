# Milestone 001: Local Agent Vertical Slice

This milestone turns the planning documents into a working Rust foundation.

## What Works

- Load an agent from TOML.
- Run a task through an Ollama or OpenAI-compatible local model.
- Ask for approval before native tools execute.
- Read files inside a workspace boundary.
- Inspect git status inside the workspace.
- Store sessions, messages, events, and tool calls in SQLite.
- Inspect recent events from the CLI.
- Save/list agents and expose agent cards.
- Search/write persistent local memory.
- Register stdio MCP servers, list tools, and call enabled MCP tools.
- Delegate from one saved agent to another with `agent_call`.
- Serve a loopback-only JSON API for agents, sessions, events, MCP server listings, and blocking agent runs.
- Persist tool approval requests and decisions.
- Wait for API approval decisions during loopback server-driven runs.
- Cancel sessions through the CLI or loopback API.
- Run deterministic runtime tests through a mock model provider.
- Redact common secret shapes before persisting logs, tool calls, approvals, messages, and memory.
- Block common destructive shell command patterns before shell execution.
- Serve the static mission-control UI from the loopback server.

## Commands

```powershell
cargo run -p agent1-cli -- run --agent agents/assistant.toml --task "Explain the project"
```

```powershell
cargo run -p agent1-cli -- events
```

```powershell
cargo run -p agent1-cli -- models --provider ollama
```

## Model Response Protocol

Final answer:

```json
{"final":"answer text"}
```

Tool call:

```json
{"tool_call":{"name":"file_read","input":{"path":"README.md"}}}
```

## Next Build Units

- Complete the migration from static mission-control viewer to Tauri/React desktop and retire the legacy static shell.
- Extend deterministic regression coverage across MCP, memory, HTTP, and security policy paths.
- Follow `docs/REMAINING_RELEASE_PLAN.md` for the Axum, WebSocket, Tauri, and release packaging work.
- Harden MCP subprocess lifecycle management for long-running servers.
