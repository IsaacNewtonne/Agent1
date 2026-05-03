# External Events and WebSocket Schema

## WebSocket Endpoint

```text
ws://127.0.0.1:17371/ws/events
```

## Subscribe Message

```json
{
  "type": "subscribe",
  "session_id": "sess_01"
}
```

## Event Envelope

```json
{
  "id": "evt_01",
  "session_id": "sess_01",
  "agent_id": "assistant",
  "type": "tool_call_requested",
  "created_at": "2026-01-01T00:00:00Z",
  "payload": {}
}
```

## Event Types

| Event Type | Description |
|---|---|
| `session_started` | New session started |
| `run_started` | Agent run started |
| `prompt_built` | Prompt constructed |
| `model_request_started` | Model call started |
| `model_response_delta` | Streaming output chunk |
| `model_response_completed` | Model call completed |
| `tool_call_requested` | Agent requested a tool |
| `tool_approval_required` | User approval required |
| `tool_call_started` | Tool execution started |
| `tool_call_completed` | Tool execution completed |
| `tool_call_failed` | Tool execution failed |
| `memory_read` | Memory was read |
| `memory_write_requested` | Memory write requested |
| `memory_written` | Memory written |
| `agent_handoff_requested` | Agent requested another agent |
| `agent_handoff_started` | Handoff started |
| `agent_handoff_completed` | Handoff completed |
| `artifact_created` | Artifact created |
| `run_completed` | Run completed |
| `run_cancelled` | Run cancelled |
| `run_failed` | Run failed |

## Tool Approval Event Payload

```json
{
  "approval_id": "approval_01",
  "tool_name": "shell_command",
  "agent_id": "code-reviewer",
  "input": {
    "command": "cargo check"
  },
  "risk_level": "medium",
  "reason": "The agent wants to check whether the Rust project compiles."
}
```

## Model Delta Payload

```json
{
  "text": "Partial streamed output"
}
```

## Tool Result Payload

```json
{
  "tool_call_id": "tool_01",
  "tool_name": "file_read",
  "status": "completed",
  "output": {
    "content_preview": "..."
  }
}
```

## Handoff Payload

```json
{
  "from_agent_id": "host",
  "to_agent_id": "critic",
  "task": "Review the draft build plan for missing risks."
}
```
