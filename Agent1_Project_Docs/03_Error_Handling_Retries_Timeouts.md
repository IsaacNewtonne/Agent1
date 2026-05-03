# Error Handling, Retries, and Timeouts

## Error Categories

| Category | Example |
|---|---|
| Config error | Invalid TOML |
| Model error | Local model endpoint unavailable |
| Tool error | Tool failed |
| Permission error | Tool denied |
| MCP error | MCP server crashed |
| Database error | SQLite write failed |
| Runtime error | Max iterations reached |
| Validation error | Invalid tool input |
| Cancellation | User cancelled run |

## Standard Error Shape

```rust
pub struct Agent1Error {
    pub code: String,
    pub message: String,
    pub details: serde_json::Value,
}
```

## Retry Policy

| Operation | Retry? | Policy |
|---|---|---|
| Model request | Yes | 2 retries with backoff |
| Tool execution | No by default | Retry only if tool marks retryable |
| SQLite write | Yes | 3 short retries |
| MCP initialize | Yes | 1 retry |
| MCP tool call | Optional | Depends on tool |
| Shell command | No | Never auto-retry |
| File write | No | Never auto-retry |

## Timeout Defaults

| Operation | Timeout |
|---|---:|
| Model request | 120 seconds |
| Streaming idle | 30 seconds |
| Tool call | 30 seconds |
| Shell command | 60 seconds |
| MCP initialize | 10 seconds |
| MCP call | 30 seconds |
| Agent run | 600 seconds |
| UI approval wait | No hard timeout by default |

## Malformed Model Output

When the model returns malformed tool call JSON:

1. Emit `model_output_malformed`.
2. Ask the model once to repair the JSON.
3. If still invalid, return a structured error to the agent.
4. Stop after configured repair attempts.

## Max Iterations

If an agent exceeds max iterations:

1. Emit `max_iterations_reached`.
2. Save partial state.
3. Return explanation to user.
4. Do not continue automatically.

## Tool Error Handling

Tool errors must return:

```json
{
  "status": "failed",
  "error_code": "file_not_found",
  "message": "The requested file does not exist."
}
```

The agent can then decide whether to recover.

## User-Facing Errors

Errors should include:

- What failed
- Why it likely failed
- What the user can do next
- Whether data was saved
