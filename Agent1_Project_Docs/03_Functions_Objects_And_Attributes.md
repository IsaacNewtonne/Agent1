# Functions, Objects, and Attributes

## Core Domain Objects

## Agent

```rust
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub description: Option<String>,
    pub role: Option<String>,
    pub system_prompt: String,
    pub model: ModelConfig,
    pub tools: Vec<ToolRef>,
    pub memory: MemoryConfig,
    pub permissions: PermissionPolicy,
    pub max_iterations: u32,
}
```

## ModelConfig

```rust
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub context_window: u32,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}
```

## Session

```rust
pub struct Session {
    pub id: SessionId,
    pub title: Option<String>,
    pub root_agent_id: AgentId,
    pub status: SessionStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

## Message

```rust
pub struct Message {
    pub id: MessageId,
    pub session_id: SessionId,
    pub from_agent_id: Option<AgentId>,
    pub to_agent_id: Option<AgentId>,
    pub role: MessageRole,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
}
```

## ToolCall

```rust
pub struct ToolCall {
    pub id: ToolCallId,
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: ToolCallStatus,
    pub error: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}
```

## RuntimeEvent

```rust
pub struct RuntimeEvent {
    pub id: EventId,
    pub session_id: Option<SessionId>,
    pub agent_id: Option<AgentId>,
    pub event_type: EventType,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}
```

## AgentCard

```rust
pub struct AgentCard {
    pub id: AgentId,
    pub name: String,
    pub description: String,
    pub skills: Vec<AgentSkill>,
    pub input_modes: Vec<String>,
    pub output_modes: Vec<String>,
    pub endpoint: String,
}
```

## AgentSkill

```rust
pub struct AgentSkill {
    pub name: String,
    pub description: String,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}
```

## Important Functions

## run_agent_session

Runs one agent session.

Inputs:

- agent ID
- session ID
- user input
- optional parent task ID

Outputs:

- final response
- artifact references
- event IDs

## build_prompt

Builds the prompt from:

- agent system instruction
- session history
- memory results
- tool definitions
- current user task

## parse_model_response

Extracts:

- final text
- tool call
- handoff request
- malformed response error

## execute_tool_call

Executes a tool after permission approval.

## call_agent

Calls another local agent through the internal agent message system.

## store_event

Writes event to SQLite and broadcasts to subscribers.
