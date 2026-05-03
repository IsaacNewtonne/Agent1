# User Stories and Acceptance Criteria

## Agent Creation

### Story

As a user, I want to create an agent with a name, model, instructions, tools, and permissions so that I can run specialized local workflows.

### Acceptance Criteria

- User can create an agent from UI.
- User can create an agent from TOML.
- Agent is stored in SQLite.
- Agent appears in dashboard.
- Invalid config returns readable errors.

## Local Model Connection

### Story

As a user, I want to connect to Ollama so that I can run agents without paid APIs.

### Acceptance Criteria

- User can enter Ollama base URL.
- User can list available models.
- User can select a model.
- Chat request works from CLI and UI.
- Failed model call shows useful error.

## Tool Approval

### Story

As a user, I want agents to ask before using sensitive tools so that my system stays safe.

### Acceptance Criteria

- File write defaults to ask.
- Shell command defaults to ask.
- User can approve or deny tool calls.
- Denied tool call is returned to the agent.
- Tool decision is logged.

## Session Trace

### Story

As a user, I want to inspect everything an agent did during a session.

### Acceptance Criteria

- Messages are stored.
- Tool calls are stored.
- Errors are stored.
- Memory reads/writes are stored.
- Events appear in UI event feed.

## MCP Tool Use

### Story

As a user, I want Agent1 to use existing MCP servers so that I can reuse community integrations.

### Acceptance Criteria

- User can add a local stdio MCP server.
- Agent1 can initialize the server.
- Agent1 can list tools.
- User can enable tools.
- Agent can call enabled tools with permission checks.

## Multi-Agent Handoff

### Story

As a user, I want one agent to delegate work to another agent so that complex tasks can be split across specialists.

### Acceptance Criteria

- Agent cards define skills.
- Host agent can call worker agent.
- Messages are stored with from/to agent IDs.
- UI shows handoff event.
- Final answer includes combined result.

## Export

### Story

As a user, I want to export session output so that I can use it outside Agent1.

### Acceptance Criteria

- Export session as Markdown.
- Export trace as JSON.
- Export generated artifacts.
