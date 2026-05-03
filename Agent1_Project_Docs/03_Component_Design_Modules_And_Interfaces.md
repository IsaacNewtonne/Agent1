# Component Design, Modules, and Interfaces

## Core Components

## Agent Store

Responsible for loading and saving agent definitions.

Interface:

```rust
#[async_trait]
pub trait AgentStore {
    async fn get(&self, id: &AgentId) -> Result<Agent>;
    async fn list(&self) -> Result<Vec<Agent>>;
    async fn save(&self, agent: Agent) -> Result<()>;
}
```

## Model Provider

Responsible for model communication.

```rust
#[async_trait]
pub trait ModelProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
}
```

## Agent Runner

Responsible for executing one session.

```rust
#[async_trait]
pub trait AgentRunner {
    async fn run(&self, request: RunAgentRequest) -> Result<RunAgentResult>;
    async fn cancel(&self, session_id: SessionId) -> Result<()>;
}
```

## Tool

Responsible for executing a callable action.

```rust
#[async_trait]
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, input: serde_json::Value, ctx: ToolContext) -> Result<ToolResult>;
}
```

## Permission Guard

Responsible for allow/ask/deny decisions.

```rust
#[async_trait]
pub trait PermissionGuard {
    async fn check(&self, request: PermissionRequest) -> Result<PermissionDecision>;
}
```

## Memory Store

Responsible for storing and retrieving memory.

```rust
#[async_trait]
pub trait MemoryStore {
    async fn search(&self, query: MemoryQuery) -> Result<Vec<MemoryItem>>;
    async fn write(&self, item: MemoryItem) -> Result<()>;
    async fn delete(&self, id: MemoryId) -> Result<()>;
}
```

## Event Sink

Responsible for publishing and storing runtime events.

```rust
#[async_trait]
pub trait EventSink {
    async fn emit(&self, event: RuntimeEvent) -> Result<()>;
}
```

## MCP Client

Responsible for connecting to an MCP server and adapting tools.

```rust
#[async_trait]
pub trait McpClient {
    async fn initialize(&self) -> Result<()>;
    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>>;
    async fn call_tool(&self, name: &str, input: serde_json::Value) -> Result<serde_json::Value>;
}
```

## Agent Registry

Responsible for discovering agents and their cards.

```rust
#[async_trait]
pub trait AgentRegistry {
    async fn get_card(&self, agent_id: &AgentId) -> Result<AgentCard>;
    async fn find_by_skill(&self, skill: &str) -> Result<Vec<AgentCard>>;
}
```
