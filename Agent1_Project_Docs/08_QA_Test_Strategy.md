# QA Test Strategy

## Test Goals

Ensure Agent1 is:

- Safe
- Reliable
- Local-first
- Cross-platform
- Correctly permissioned
- Observable
- Usable

## Test Levels

## Unit Tests

Cover:

- Config parsing
- Agent validation
- Permission decisions
- Tool schema validation
- Prompt building
- Event serialization
- Error mapping

## Integration Tests

Cover:

- SQLite repositories
- Agent run loop with mock model
- Tool execution
- Permission approval flow
- MCP client with mock server
- WebSocket event stream

## End-to-End Tests

Cover:

- CLI creates session
- CLI runs agent
- UI creates agent
- UI approves tool call
- UI shows event feed
- MCP tool is discovered and called

## Security Tests

Cover:

- Shell command denylist
- Path traversal attempts
- Workspace boundary enforcement
- Secret redaction
- Prompt injection scenarios
- MCP untrusted tool behavior

## Performance Tests

Cover:

- Large session loading
- Event stream volume
- Database growth
- Tool output size limits
- UI responsiveness

## Compatibility Tests

Platforms:

- Windows 10/11
- Ubuntu LTS
- macOS

Model providers:

- Ollama
- OpenAI-compatible local endpoint mock

## Mock Model

Create a deterministic mock model provider for tests.

It should return:

- Normal final response
- Valid tool call
- Malformed tool call
- Agent handoff request
- Long response
- Error response
