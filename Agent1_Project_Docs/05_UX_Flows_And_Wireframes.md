# UX Flows and Wireframes

## First Run Flow

```text
Welcome
↓
Detect Ollama/local endpoint
↓
Select model
↓
Create first agent from template
↓
Run test prompt
↓
Open dashboard
```

## Create Agent Flow

```text
Dashboard
↓
New Agent
↓
Choose template
↓
Edit name/instructions/model
↓
Select tools
↓
Set permissions
↓
Save
↓
Run test
```

## Run Task Flow

```text
Select agent
↓
Enter task
↓
Start run
↓
Watch streaming output
↓
Approve or deny tool requests
↓
Review final answer
↓
Inspect event trace
```

## MCP Setup Flow

```text
Settings
↓
MCP Servers
↓
Add server
↓
Enter command/args/env
↓
Test connection
↓
Review discovered tools
↓
Enable selected tools
↓
Assign to agent
```

## Multi-Agent Flow

```text
Create host agent
↓
Add planner/worker/critic agents
↓
Assign skills
↓
Open agent graph
↓
Run task
↓
Watch handoffs in graph
```

## Wireframe: Dashboard

```text
┌──────────────────────────────────────────────┐
│ Agent1                                       │
├──────────────────────────────────────────────┤
│ Model Status: Ollama connected               │
│ Active Runs: 0                               │
│                                              │
│ [New Agent] [Run Task] [Add MCP Server]      │
│                                              │
│ Recent Sessions                              │
│ - Code review                                │
│ - Build plan                                 │
│                                              │
│ Agents                                       │
│ - Assistant                                  │
│ - Code Reviewer                              │
│ - Planner                                    │
└──────────────────────────────────────────────┘
```

## Wireframe: Session

```text
┌───────────────┬─────────────────────┬──────────────┐
│ Agents        │ Session             │ Events       │
├───────────────┼─────────────────────┼──────────────┤
│ Host          │ User: Review repo   │ run_started  │
│ ├ Planner     │ Agent: I need files │ tool_request │
│ ├ Worker      │ [Approval Card]     │ approved     │
│ └ Critic      │ Agent: Results...   │ completed    │
└───────────────┴─────────────────────┴──────────────┘
```

## Wireframe: Approval

```text
┌──────────────────────────────────────┐
│ Tool Approval Required               │
├──────────────────────────────────────┤
│ Agent: Code Reviewer                 │
│ Tool: shell_command                  │
│ Command: cargo check                 │
│ Risk: Medium                         │
│                                      │
│ [Deny] [Approve once] [Allow session]│
└──────────────────────────────────────┘
```
