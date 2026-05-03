# Runbook and Operations

## Common Operations

## Start Desktop App

Run Agent1 from installed desktop shortcut or:

```bash
agent1 desktop
```

## Start Local Server

```bash
agent1 server start
```

## Run Agent from CLI

```bash
agent1 run --agent assistant --task "Hello"
```

## Check Model Provider

```bash
agent1 models list
```

## Check Database

```bash
agent1 doctor
```

## Backup

```bash
agent1 backup create
```

## Restore

```bash
agent1 backup restore ./agent1-backup.zip
```

## Troubleshooting

## Ollama Not Connected

Symptoms:

- Model list empty
- Model request fails

Steps:

1. Check Ollama is running.
2. Check base URL in config.
3. Run `agent1 models list`.
4. Review logs.

## Agent Stuck Running

Steps:

1. Press cancel in UI.
2. Run `agent1 sessions cancel <session_id>`.
3. Check tool timeout.
4. Check model provider availability.

## Tool Approval Not Appearing

Steps:

1. Check WebSocket connection.
2. Check event log for `tool_approval_required`.
3. Restart desktop UI.
4. Confirm permission is set to `ask`.

## MCP Server Fails

Steps:

1. Test MCP command manually.
2. Check command path.
3. Check args and env.
4. Review MCP logs.
5. Disable server if unsafe.

## Database Corruption

Steps:

1. Stop Agent1.
2. Backup current data directory.
3. Run SQLite integrity check.
4. Restore latest backup if needed.

## Logs Location

```text
~/.agent1/logs/
```
