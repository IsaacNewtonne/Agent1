# Decision Log and Architecture Decision Records

## ADR-001: Use Rust as the primary implementation language

### Status

Accepted

### Context

Agent1 is intended to be fast, reliable, secure, and suitable for local execution.

### Decision

Use Rust for the backend, runtime, CLI, server, tool execution, persistence layer, MCP client, and A2A components.

### Consequences

- Strong safety and performance.
- More work needed for AI ecosystem integrations because many examples are Python-first.
- Better long-term maintainability for a local agent runtime.

---

## ADR-002: Use SQLite as the default database

### Status

Accepted

### Context

Agent1 is local-first and must work without a separate database server.

### Decision

Use SQLite for agents, sessions, messages, tool calls, events, tasks, memory, and configuration.

### Consequences

- Easy installation.
- Portable user data.
- Good enough for MVP and desktop use.
- Server deployments can add PostgreSQL later if needed.

---

## ADR-003: Use Ollama as the first model backend

### Status

Accepted

### Context

The MVP needs a simple local model runner.

### Decision

Support Ollama first, but implement it behind a `ModelProvider` trait.

### Consequences

- Fast MVP path.
- Easy for users to test.
- Avoids vendor lock-in by isolating Ollama in an adapter.

---

## ADR-004: Use Tauri for desktop app

### Status

Accepted

### Context

Agent1 needs a desktop UI with local system access and Rust integration.

### Decision

Use Tauri for the desktop application.

### Consequences

- Rust backend integration is natural.
- Smaller app footprint than Electron.
- Frontend can be implemented with React or Svelte.

---

## ADR-005: Permissioned tools by default

### Status

Accepted

### Context

Agents with local file and shell access can damage a user's machine.

### Decision

All dangerous tools must default to ask or deny.

### Consequences

- Safer default behavior.
- More user approval prompts in early versions.
- Later versions can add trusted workspaces and policies.

---

## ADR-006: MCP client first, MCP server later

### Status

Accepted

### Context

The fastest way to benefit from MCP is to connect to existing MCP servers.

### Decision

Build Agent1 as an MCP client first. Expose Agent1 tools as an MCP server later.

### Consequences

- Faster integration.
- Smaller MVP.
- Later plugin ecosystem remains possible.

---

## ADR-007: A2A-inspired local implementation first

### Status

Accepted

### Context

Agent-to-agent standards are evolving. Agent1 needs agent collaboration without depending on cloud services.

### Decision

Implement local agent cards and local task messaging first. Add stronger remote compatibility after MVP.

### Consequences

- Useful multi-agent orchestration in MVP.
- Avoids overbuilding network protocol complexity early.
