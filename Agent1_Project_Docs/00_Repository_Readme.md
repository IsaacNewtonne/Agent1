# Agent1

Agent1 is a self-hosted, open-source AI agent platform written in Rust.

It allows users to create and run local AI agents using local models, local tools, local memory, MCP-compatible integrations, and A2A-style agent communication.

Agent1 does not require OpenAI, Claude, Gemini, Vertex AI, or any hosted API provider.

## Core Features

- Rust-native agent runtime
- Local model support through Ollama and OpenAI-compatible local endpoints
- Configurable agents with instructions, tools, memory, and permissions
- Safe local tool execution
- MCP client support for local MCP servers
- A2A-style agent cards and agent-to-agent communication
- SQLite persistence
- Full session trace logging
- Tauri desktop UI
- CLI for developers
- Local-first security model

## Recommended Repository Structure

```text
agent1/
├── Cargo.toml
├── crates/
│   ├── agent1-core/
│   ├── agent1-runtime/
│   ├── agent1-models/
│   ├── agent1-tools/
│   ├── agent1-memory/
│   ├── agent1-mcp/
│   ├── agent1-a2a/
│   ├── agent1-db/
│   ├── agent1-server/
│   ├── agent1-cli/
│   └── agent1-common/
├── apps/
│   └── desktop/
├── agents/
│   ├── assistant.toml
│   ├── code_reviewer.toml
│   └── planner.toml
├── docs/
├── examples/
└── tests/
```

## First Developer Milestone

```bash
agent1 run \
  --agent ./agents/code_reviewer.toml \
  --task "Review this Rust repository and explain the architecture"
```

Expected result:

- Agent config loads from TOML.
- Ollama/local model responds.
- Agent may request safe tool approval.
- Session is saved to SQLite.
- Event trace is visible through CLI and UI.

## Development Principles

1. Do not hardcode a single model provider.
2. Do not allow tools to run silently by default.
3. Do not require hosted APIs.
4. Do not hide agent actions from the user.
5. Do not make UI features depend on cloud services.
6. Prefer Rust traits and adapters over vendor-specific logic.
7. Store user data locally by default.
8. Make every agent run inspectable and reproducible where possible.

## Minimum Requirements

- Rust stable
- Tokio
- SQLite
- Ollama or another local model endpoint
- Tauri for desktop UI
- Node.js only for frontend build tooling
