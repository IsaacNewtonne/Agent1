# Configuration and Feature Flags

## App Config Location

Linux/macOS:

```text
~/.agent1/config.toml
```

Windows:

```text
%APPDATA%/Agent1/config.toml
```

## Example App Config

```toml
[app]
data_dir = "~/.agent1"
log_level = "info"
first_run_complete = false

[server]
enabled = true
host = "127.0.0.1"
port = 17371

[models.ollama]
enabled = true
base_url = "http://localhost:11434"

[security.defaults]
file_read = "ask"
file_write = "ask"
shell = "ask"
network = "deny"
memory_write = "ask"

[runtime]
max_iterations = 12
max_tool_calls = 20
max_runtime_seconds = 600

[features]
mcp_client = true
a2a_local = true
desktop_ui = true
vector_memory = false
remote_agents = false
browser_automation = false
telemetry = false
```

## Agent Config Example

```toml
id = "code-reviewer"
name = "Code Reviewer"
description = "Reviews Rust code for architecture, bugs, and maintainability."
role = "Senior Rust code reviewer"
max_iterations = 12

[model]
provider = "ollama"
name = "qwen3.5:4b"
context_window = 65536
temperature = 0.2

[instructions]
system = '''
You are a senior Rust code reviewer.
Inspect projects carefully.
Do not modify files unless the user approves.
'''

[permissions]
file_read = "ask"
file_write = "ask"
shell = "ask"
network = "deny"

[tools]
enabled = ["file_read", "git_status", "cargo_check"]

[memory]
enabled = true
write_policy = "ask"
```

## Feature Flags

| Flag | Default | Description |
|---|---:|---|
| `mcp_client` | true | Enable MCP client |
| `a2a_local` | true | Enable local agent cards and handoffs |
| `desktop_ui` | true | Enable desktop UI |
| `vector_memory` | false | Enable embedding search |
| `remote_agents` | false | Enable remote agent calls |
| `browser_automation` | false | Enable browser tools |
| `telemetry` | false | Enable optional telemetry |

## Permission Values

```text
allow
ask
deny
```

## Environment Variables

| Variable | Purpose |
|---|---|
| `AGENT1_CONFIG` | Override config path |
| `AGENT1_DATA_DIR` | Override data directory |
| `AGENT1_LOG` | Override log level |
| `AGENT1_DB_URL` | Override SQLite DB URL |
