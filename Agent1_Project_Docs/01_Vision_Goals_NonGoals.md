# Vision, Goals, and Non-Goals

## Vision

Agent1 should become the local-first operating layer for open-source AI agents.

Users should be able to build, run, inspect, and connect agents without trusting a cloud provider or paying for hosted APIs.

## Product Positioning

Agent1 is not a chatbot wrapper.

Agent1 is:

- A Rust agent runtime
- A local model orchestration layer
- A permissioned tool execution system
- A local memory system
- An MCP client
- An A2A-style agent collaboration layer
- A desktop and CLI control panel for local agents

## Goals

### Product Goals

1. Make local agents practical for everyday developer workflows.
2. Make every agent action visible.
3. Make local model use simple.
4. Make tool use safe.
5. Make agent composition understandable through visual graphs.
6. Make the system extensible by open-source contributors.

### Technical Goals

1. Rust-first architecture.
2. Minimal runtime dependencies.
3. SQLite-based local persistence.
4. Trait-based provider abstraction.
5. Modular crate design.
6. Strong typed schemas.
7. Structured event tracing.
8. Safe defaults.

### Community Goals

1. Easy to install.
2. Easy to contribute.
3. Well-documented.
4. Plugin-friendly.
5. Useful examples.

## Non-Goals

Agent1 will not initially support:

- Cloud-hosted agent execution
- Paid API-first workflows
- Autonomous unrestricted computer control
- Browser control without sandboxing
- Multi-user enterprise admin
- Marketplace payments
- Proprietary agent formats
- Mandatory telemetry
