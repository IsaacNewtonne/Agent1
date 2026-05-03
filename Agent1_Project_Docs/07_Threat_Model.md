# Threat Model

## Assets

- User files
- Source code
- Local database
- Agent memories
- Secrets
- Shell access
- MCP server access
- Generated artifacts
- Session history

## Trust Boundaries

```text
User
↓
Agent1 UI
↓
Agent1 Runtime
↓
Tools / MCP Servers / Local Model
↓
Filesystem / Shell / Network
```

## Threats

## T-001: Prompt Injection from Files

A file contains instructions telling the agent to ignore rules or exfiltrate data.

### Mitigation

- Treat file contents as untrusted.
- Prompt model that files are data, not instructions.
- Do not allow network by default.
- Require approval for sensitive tools.

## T-002: Dangerous Shell Command

Agent requests a destructive command.

### Mitigation

- Shell tool defaults to ask.
- Show exact command.
- Add denylist for destructive commands.
- Run with timeout.
- Support dry-run where possible.

## T-003: Malicious MCP Server

MCP server exposes dangerous tools or returns malicious content.

### Mitigation

- Require explicit server enablement.
- Require per-tool enablement.
- Log all MCP calls.
- Apply same permission guard to MCP tools.

## T-004: Secret Leakage in Logs

Tool output includes secrets.

### Mitigation

- Redact common secret formats.
- Avoid logging environment variables.
- Allow user to clear logs.
- Mark sensitive tool outputs.

## T-005: Unauthorized Local API Access

Another local process calls Agent1 API.

### Mitigation

- Bind to localhost.
- Use local auth token in server mode.
- Do not expose public bind by default.
- Warn user before enabling public bind.

## T-006: Infinite Agent Loop

Agent repeatedly calls tools.

### Mitigation

- Max iterations.
- Max tool calls.
- Max runtime seconds.
- User cancellation.

## T-007: Data Loss Through File Write

Agent overwrites important files.

### Mitigation

- File write asks by default.
- Show diff before write where possible.
- Write to workspace only.
- Backup overwritten files.
