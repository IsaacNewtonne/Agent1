# Async Runtime and Concurrency

## Runtime

Agent1 uses Tokio as the async runtime.

## Concurrency Goals

- Run model requests without blocking UI.
- Run tools with timeouts.
- Stream events to UI.
- Manage MCP subprocesses.
- Allow cancellation of agent runs.
- Prevent unbounded agent loops.

## Main Async Components

| Component | Concurrency Role |
|---|---|
| Agent runner | Owns run loop for one session |
| Tool runner | Executes tool calls with timeout |
| Event bus | Broadcasts events to UI/server |
| MCP manager | Manages subprocess I/O |
| Model provider | Performs HTTP requests |
| Database repository | Persists events/messages safely |

## Cancellation

Each agent run must have a cancellation token.

Cancellation should stop:

- Pending model request where possible
- Waiting tool call
- Future loop iterations
- Streaming to UI

Cancellation should not corrupt:

- Existing session data
- Tool call records
- Event records

## Timeouts

| Operation | Default Timeout |
|---|---:|
| Model request | 120 seconds |
| Tool call | 30 seconds |
| Shell command | 60 seconds |
| MCP initialize | 10 seconds |
| MCP tool call | 30 seconds |
| Database write | 5 seconds |

## Agent Loop Protection

Every agent config must include:

```toml
max_iterations = 12
max_tool_calls = 20
max_runtime_seconds = 600
```

## Event Bus

Events should be sent through an internal broadcast channel.

```text
Runtime emits event
↓
Database event writer stores event
↓
UI subscriber receives event
↓
CLI subscriber prints event
```

## Blocking Work

Blocking filesystem or process operations must run through safe wrappers and not block the async runtime unnecessarily.

## Process Isolation

Shell tools and MCP servers run as child processes with:

- Working directory restrictions
- Environment filtering
- Timeout
- Output size limit
- Kill-on-cancel behavior
