# Use Cases

## UC-001: Create a Local Assistant

A user creates a general-purpose assistant using a local model.

### Flow

1. User opens Agent1.
2. User selects local model provider.
3. User creates an agent.
4. User enters system instructions.
5. User runs a chat.
6. Agent responds.
7. Session is saved.

## UC-002: Review a Codebase

A developer asks Agent1 to inspect a local Rust project.

### Flow

1. User selects a workspace folder.
2. User runs the Code Reviewer agent.
3. Agent requests permission to read files.
4. User approves.
5. Agent summarizes architecture and issues.
6. Agent suggests fixes.
7. Event log records all files read.

## UC-003: Run a Safe Shell Command

An agent wants to run `cargo check`.

### Flow

1. Agent proposes shell command.
2. Runtime checks permissions.
3. User sees approval modal.
4. User approves or denies.
5. Command runs with timeout.
6. Output is returned to agent.
7. Tool call is logged.

## UC-004: Use an MCP Server

A user connects an MCP filesystem or git server.

### Flow

1. User adds MCP server config.
2. Agent1 starts the MCP server process.
3. Agent1 lists available MCP tools.
4. User enables selected tools.
5. Agent calls MCP tool through permission guard.
6. Result is stored in trace.

## UC-005: Multi-Agent Planning

A user asks for a build plan.

### Flow

1. Host agent receives request.
2. Planner agent creates tasks.
3. Worker agent expands tasks.
4. Critic agent reviews output.
5. Host agent returns final response.

## UC-006: Inspect Agent Behavior

A user wants to understand why an agent produced an answer.

### Flow

1. User opens session trace.
2. User reviews messages, model responses, tool calls, memory reads, and memory writes.
3. User exports the session as Markdown or JSON.
