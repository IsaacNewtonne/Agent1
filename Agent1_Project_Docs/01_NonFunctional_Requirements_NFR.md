# Non-Functional Requirements

## Performance

- CLI startup should complete in under 1 second excluding model startup.
- UI should load dashboard in under 2 seconds for normal local databases.
- Tool approval UI should appear within 500 ms after a tool request event.
- Agent run loop must support streaming model output where provider supports it.
- SQLite queries should be indexed for sessions, messages, events, and tool calls.

## Reliability

- Agent run loop must survive tool errors.
- Tool timeout must not crash the runtime.
- Model failure must produce structured error.
- SQLite writes must be transactional for session-critical data.
- Background MCP process failure must be visible in event log.

## Security

- Dangerous tools default to ask or deny.
- Shell commands require explicit approval unless allowlisted.
- File access is restricted to approved workspaces.
- Network access is disabled by default.
- Tool input must be schema-validated.
- Secrets must not be logged.

## Privacy

- No mandatory telemetry.
- Local storage by default.
- User can delete sessions, memory, and logs.
- Export must be user-initiated.
- Remote model providers are not part of MVP.

## Portability

- Must support Windows, Linux, and macOS.
- Must support local desktop mode.
- Must support headless server mode later.
- Config should be TOML/JSON, not hidden binary format.

## Maintainability

- Modular crate design.
- Traits for model providers and tools.
- Strong typed events.
- Migration-based database changes.
- Unit tests for core logic.
- Integration tests for runtime behavior.

## Usability

- First run should guide user to configure a local model.
- Errors should explain the fix.
- Tool approvals should clearly show what the agent wants to do.
- Agent graph should make handoffs understandable.
