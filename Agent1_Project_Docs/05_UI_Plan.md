# UI Plan

## UI Goal

The UI should make local agents understandable, safe, and controllable.

It must show:

- What agent is running
- What model it uses
- What tools it wants
- What it has done
- What memory it used or wrote
- What other agents it called

## Main Layout

```text
┌──────────────────────────────────────────────────────────┐
│ Top Bar: Agent1 | Model | Workspace | Run Status         │
├───────────────┬─────────────────────────────┬────────────┤
│ Agent Tree    │ Session Workspace           │ Event Feed │
│               │                             │            │
│ Host Agent    │ Chat/task thread            │ Tool calls │
│ ├ Planner     │ Artifacts                    │ Errors     │
│ ├ Worker      │ Approvals                    │ Memory     │
│ └ Critic      │ Final output                 │ Handoffs   │
└───────────────┴─────────────────────────────┴────────────┘
```

## Screens

## Dashboard

Shows:

- Local model status
- Recent sessions
- Active runs
- Agents
- MCP server status
- Quick start actions

## Agent Builder

Fields:

- Name
- Description
- Role
- System instruction
- Model provider
- Model name
- Temperature
- Context window
- Tools
- Permissions
- Memory settings
- Skills

## Session Workspace

Shows:

- User messages
- Agent responses
- Tool results
- Artifacts
- Streaming output
- Current run status

## Event Feed

Shows:

- Model calls
- Tool calls
- Approval requests
- Memory reads/writes
- Handoffs
- Errors
- Completion

## Tool Approval Modal

Must show:

- Agent requesting action
- Tool name
- Exact input
- Risk level
- Workspace affected
- Approve/Deny buttons
- Optional allow for this session

## MCP Manager

Shows:

- Configured MCP servers
- Running/stopped status
- Available tools
- Enabled/disabled tools
- Last error

## Agent Graph

Shows:

- Agents as nodes
- Calls/handoffs as edges
- Active agent highlighted
- Tool usage as side events

## Settings

Includes:

- Model providers
- Data directory
- Default permissions
- Memory settings
- Log settings
- Backup/export
