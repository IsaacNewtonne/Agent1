# Observability Spec

## Observability Goal

Users must be able to see what Agent1 is doing and why.

## Observable Items

- Session start/end
- Agent run start/end
- Prompt construction
- Model request/response
- Tool request
- Tool approval
- Tool execution
- Tool result
- Memory read/write
- Agent handoff
- Artifact creation
- Errors
- Cancellation

## Logs

Use structured logs through Rust tracing.

Log levels:

| Level | Use |
|---|---|
| error | Failures requiring attention |
| warn | Recoverable problems |
| info | Normal lifecycle events |
| debug | Developer details |
| trace | Very detailed diagnostics |

## Event Store

Events are stored in SQLite.

Each event includes:

- ID
- session ID
- agent ID
- event type
- payload
- timestamp

## UI Event Feed

The event feed should support:

- Filter by event type
- Filter by agent
- Expand/collapse event details
- Copy event JSON
- Jump to related message/tool call

## Metrics

Local-only metrics for debugging:

- Total sessions
- Average run duration
- Tool calls per run
- Error count
- Model request duration
- Tool duration
- MCP server uptime

## Trace Export

Users can export trace as JSON.

Trace export should include:

- Session metadata
- Messages
- Events
- Tool calls
- Artifacts
- Redacted sensitive values
