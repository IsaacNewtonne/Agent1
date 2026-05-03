# Security, Privacy, and Compliance Plan

## Security Principles

- Local-first by default.
- Network off by default for agents.
- Tools permissioned by default.
- Dangerous actions require approval.
- Every tool call logged.
- Secrets redacted.
- MCP servers treated as untrusted.

## Threat Areas

| Area | Risk |
|---|---|
| Shell tool | System damage |
| File write tool | Data loss |
| File read tool | Sensitive file exposure |
| MCP server | Malicious or buggy tool |
| Prompt injection | Agent follows malicious file instructions |
| Memory | Sensitive data stored long-term |
| Logs | Secrets leaked |
| Local API | Unauthorized local access |

## Required Controls

### Tool Permissions

Each agent has a permission policy:

```text
allow
ask
deny
```

Dangerous defaults:

| Tool Type | Default |
|---|---|
| File read | Ask |
| File write | Ask |
| Shell | Ask |
| Network | Deny |
| Delete | Deny |
| Memory write | Ask |

### Workspace Boundary

Agents can only access approved workspace folders.

### Approval UI

Approval prompts must show:

- Agent
- Tool
- Exact input
- Target files/commands
- Risk level
- Consequences

### Secret Redaction

Redact:

- API keys
- Tokens
- Passwords
- Private keys
- SSH keys
- `.env` values

### Local API Security

- Bind to `127.0.0.1` by default.
- Require token for non-desktop server mode.
- Do not expose public network access unless explicitly configured.

## Privacy

- No mandatory telemetry.
- No cloud sync in MVP.
- No external model APIs in default templates.
- User can delete memories and sessions.
- User can inspect all stored data.

## Compliance Notes

Agent1 MVP is a local open-source tool, not a hosted data processor.

For organizations, compliance depends on deployment mode, enabled tools, model backend, and data used.
