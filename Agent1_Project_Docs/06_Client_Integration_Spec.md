# Client Integration Spec

## Integration Types

Agent1 exposes three main client surfaces:

1. CLI
2. Local HTTP API
3. Desktop UI through Tauri commands

## CLI Commands

### agent1 models list

Lists models from configured local providers.

```bash
agent1 models list
```

### agent1 agent create

Creates an agent from TOML.

```bash
agent1 agent create ./agents/code_reviewer.toml
```

### agent1 run

Runs an agent.

```bash
agent1 run --agent code-reviewer --task "Review this repo"
```

### agent1 mcp add

Adds an MCP server.

```bash
agent1 mcp add --name filesystem --command npx --args "@modelcontextprotocol/server-filesystem ./"
```

### agent1 sessions list

Lists recent sessions.

```bash
agent1 sessions list
```

### agent1 export

Exports a session.

```bash
agent1 export session sess_01 --format markdown
```

## Local HTTP API

Used by desktop UI and optional local integrations.

Default bind:

```text
127.0.0.1:17371
```

The local server must not bind to public interfaces unless explicitly configured.

## Tauri Commands

Suggested commands:

```text
get_app_status
list_agents
create_agent
update_agent
delete_agent
list_sessions
create_session
run_agent
cancel_run
approve_tool_call
deny_tool_call
list_models
list_mcp_servers
add_mcp_server
test_mcp_server
list_events
export_session
```

## Local App Integration

Other apps can discover local agents using:

```text
GET /.well-known/agent.json
```

and submit tasks using local API endpoints.

## Authentication

MVP local desktop mode can use loopback-only access.

For server mode:

- Require local auth token.
- Store token securely.
- Reject unauthenticated requests.
