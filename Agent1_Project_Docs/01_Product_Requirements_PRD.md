# Product Requirements Document: Agent1

## Product Summary

Agent1 is an open-source, self-hosted, Rust-first platform for creating, running, connecting, observing, and governing AI agents locally.

It is designed for users who want agent workflows without hosted AI APIs, cloud lock-in, or hidden tool execution.

## Problem

Current agent stacks are often:

- Cloud-dependent
- API-cost dependent
- Fragmented across frameworks
- Difficult to inspect
- Hard to secure locally
- Weak at tool permissioning
- Weak at agent-to-agent coordination
- Difficult for non-experts to operate

## Target Users

- Developers building local AI tools
- Open-source builders
- Power users using Ollama/local models
- Small teams wanting private self-hosted agents
- Researchers testing multi-agent patterns
- Users building coding, planning, automation, and knowledge agents

## Goals

1. Let users create agents locally.
2. Let agents use local models.
3. Let agents use safe tools.
4. Let agents connect to MCP servers.
5. Let agents call other agents.
6. Let users observe all agent behavior.
7. Keep user data local by default.
8. Provide both CLI and desktop UI.

## Non-Goals

1. No hosted model marketplace in MVP.
2. No paid API requirement.
3. No cloud-only deployment.
4. No unrestricted autonomous shell access.
5. No browser automation in MVP unless sandboxed.
6. No enterprise SSO in MVP.
7. No mobile app in MVP.

## MVP Requirements

### Agent Management

- Create agent from UI and TOML config.
- Edit name, role, system prompt, model, tools, and permissions.
- Enable or disable memory per agent.

### Runtime

- Run a single agent.
- Run a basic planner-worker-critic flow.
- Store full sessions.
- Emit structured events.

### Model Providers

- Support Ollama chat endpoint.
- Support OpenAI-compatible local endpoint.
- Allow custom base URL and model name.

### Tools

- Native file read tool.
- Native file write tool.
- Native shell command tool with approval.
- Native git status/diff tool.
- Native task/todo tool.
- Native agent_call tool.

### MCP

- Add local stdio MCP server.
- List MCP tools.
- Call MCP tools with permission checks.
- Log MCP calls.

### UI

- Dashboard.
- Agent builder.
- Chat/session runner.
- Tool approval modal.
- Event feed.
- Agent graph.
- Settings.

### Security

- Permissioned tools.
- Workspace boundaries.
- Approval prompts.
- Tool call logs.
- Network off by default.

## Acceptance Criteria

Agent1 MVP is complete when:

1. A user can install Agent1 locally.
2. A user can connect to Ollama.
3. A user can create an agent.
4. A user can run a task through the agent.
5. The agent can request approval to read a file.
6. The user can approve or deny the tool call.
7. The session is stored in SQLite.
8. The event feed shows model calls, tool calls, and final output.
9. A basic MCP stdio server can be connected.
10. A host agent can call a worker agent.
