# Project Plan, Timeline, and Milestones

## Development Strategy

Build backend runtime first. Add UI only after the CLI can run real agent sessions.

## Milestone 1: Repository Foundation

### Deliverables

- Cargo workspace
- Crate layout
- Config loader
- Common error types
- Logging/tracing
- SQLite migration runner

### Exit Criteria

`cargo test` runs across workspace.

## Milestone 2: Model Provider Layer

### Deliverables

- `ModelProvider` trait
- Ollama adapter
- OpenAI-compatible local adapter
- Model listing
- Basic chat completion

### Exit Criteria

CLI can send a message to a local model and print response.

## Milestone 3: Single Agent Runtime

### Deliverables

- Agent config schema
- Prompt builder
- Session state
- Agent run loop
- SQLite session/message storage

### Exit Criteria

CLI can run one agent from TOML.

## Milestone 4: Native Tools

### Deliverables

- Tool trait
- Tool registry
- File read tool
- File write tool
- Shell command tool
- Git tools
- Permission guard

### Exit Criteria

Agent can request file read and user can approve it.

## Milestone 5: Event Trace

### Deliverables

- Event schema
- Event persistence
- Event streaming internally
- CLI trace output

### Exit Criteria

Every model call, tool call, error, and final response is recorded.

## Milestone 6: Memory

### Deliverables

- Session memory
- Long-term memory table
- Memory search
- Memory write event

### Exit Criteria

Agent can retrieve previous local memory.

## Milestone 7: MCP Client

### Deliverables

- MCP stdio process manager
- Initialize/list tools/call tool
- MCP-to-Agent1 tool adapter
- MCP permission policy

### Exit Criteria

Agent can call an enabled MCP tool.

## Milestone 8: Multi-Agent Runtime

### Deliverables

- Agent card
- Agent skill registry
- Agent call tool
- Host/planner/worker/critic example

### Exit Criteria

Host agent delegates a task to worker agent and returns result.

## Milestone 9: Desktop UI

### Deliverables

- Tauri shell
- Dashboard
- Agent builder
- Session viewer
- Approval modal
- Event feed
- Agent graph

### Exit Criteria

User can create and run an agent from desktop UI.

## Milestone 10: Release Candidate

### Deliverables

- Installers
- Docker compose
- Docs
- Examples
- Security review
- Test suite

### Exit Criteria

New user can install and run first agent in under 10 minutes.

## Current Implementation Status

As of the local implementation pass on 2026-05-02, the repository includes the CLI/runtime vertical slice plus typed storage and commands for memory, MCP stdio server configs, agent cards, persisted approvals, agent handoff, trace export, cancellation markers, deterministic mock-provider runtime tests, an Axum loopback API with request-size limits and structured error responses, optional API token authorization, a `/ws/events` stream, full duplex WebSocket approval decisions with push-signaled runtime waits for API-driven runs, a managed MCP stdio session pool replacing one-shot process-per-call MCP execution, Ollama streaming parsed into model delta events, and a functional `desktop/` React/Tauri application with save/run controls, approval actions, session explorer tabs, and successful Windows packaging (`agent1-desktop.exe`). The remaining production work is to complete desktop parity for MCP/memory/cancel flows, harden long-running MCP pool lifecycle management, extend streaming parity across providers and UI controls, and broaden deterministic regression coverage with stable CI execution.
