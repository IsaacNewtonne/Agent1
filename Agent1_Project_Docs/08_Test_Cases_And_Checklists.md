# Test Cases and Checklists

## Agent Config Tests

- [x] Valid TOML loads.
- [x] Missing name fails.
- [x] Missing model fails.
- [x] Invalid permission value fails.
- [x] Unknown tool fails with useful error.
- [x] Max iterations defaults correctly.

## Model Provider Tests

- [x] Ollama list models works.
- [x] Ollama chat works.
- [x] Endpoint unavailable returns clear error.
- [x] Timeout returns structured error.
- [x] OpenAI-compatible local endpoint works with mock.

## Runtime Tests

- [x] Agent can answer without tools.
- [x] Agent can request tool.
- [x] Tool result returns to agent.
- [x] Agent stops at final answer.
- [x] Agent stops at max iterations.
- [x] User can cancel run.
- [x] Session is saved after failure.

## Tool Tests

- [x] File read requires approval.
- [x] File write requires approval.
- [x] Shell command requires approval.
- [x] Denied tool call is recorded.
- [x] Tool timeout is handled.
- [x] Tool output size limit works.
- [x] Path traversal is blocked.
- [x] Workspace boundary is enforced.

## MCP Tests

- [x] MCP server config saves.
- [x] MCP server starts.
- [x] Initialize succeeds.
- [x] Tools are listed.
- [x] Tool schema is converted.
- [x] Tool call works.
- [x] MCP crash is logged.
- [x] Disabled MCP tool cannot be called.

## Multi-Agent Tests

- [x] Agent card generates.
- [x] Agent skill is searchable.
- [x] Host calls worker.
- [x] Handoff event is logged.
- [x] Worker result returns to host.
- [x] Critic can review worker output.

## UI Tests

- [x] Dashboard loads.
- [x] Agent builder saves agent.
- [x] Session view streams output.
- [x] Approval modal shows exact action.
- [x] Event feed updates live.
- [x] Agent graph shows handoff.
- [x] Settings save correctly.

## Security Checklist

- [x] Network disabled by default.
- [x] Shell asks by default.
- [x] File write asks by default.
- [x] Delete blocked by default.
- [x] Secrets redacted.
- [x] Local API binds to localhost.
- [x] Public bind requires explicit config.
