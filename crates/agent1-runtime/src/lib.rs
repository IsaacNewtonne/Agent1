use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, OnceLock},
};

use agent1_core::{
    Agent, Agent1Error, ApprovalRecord, ChatMessage, ChatRequest, EventType, McpServerConfig,
    MemoryItem, Message, MessageRole, PermissionMode, Result, RuntimeEvent, Session,
    SessionStatus, ToolCallRecord, ToolCallStatus, ToolDefinition, ToolResult, new_id, now,
};
use agent1_db::SqliteStore;
use agent1_models::provider_for;
use agent1_tools::{ToolContext, ToolRegistry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex, Semaphore},
    time::{Duration, timeout},
};

#[derive(Debug, Clone)]
pub struct RunAgentRequest {
    pub agent: Agent,
    pub input: String,
    pub title: Option<String>,
    pub workspace_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RunAgentResult {
    pub session_id: String,
    pub final_answer: String,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub approval_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub input: Value,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Copy)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[async_trait]
pub trait ApprovalDelegate: Send + Sync {
    async fn approve(&self, request: ApprovalRequest) -> Result<bool>;
}

pub struct AgentRuntime<A: ApprovalDelegate> {
    store: SqliteStore,
    tools: ToolRegistry,
    approvals: A,
    session_semaphore: Arc<Semaphore>,
}

impl<A: ApprovalDelegate + Clone> AgentRuntime<A> {
    pub fn new(store: SqliteStore, tools: ToolRegistry, approvals: A) -> Self {
        Self {
            store,
            tools,
            approvals,
            session_semaphore: Arc::new(Semaphore::new(4)),
        }
    }

    pub fn with_max_concurrent(
        store: SqliteStore,
        tools: ToolRegistry,
        approvals: A,
        max_concurrent: usize,
    ) -> Self {
        Self {
            store,
            tools,
            approvals,
            session_semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn run(&self, request: RunAgentRequest) -> Result<RunAgentResult> {
        let _permit = self.session_semaphore.acquire().await.map_err(|_| {
            Agent1Error::Runtime("concurrent session limit reached".to_string())
        })?;
        self.store.save_agent(&request.agent).await?;
        let session_id = new_id("sess");
        let created_at = now();
        let session = Session {
            id: session_id.clone(),
            title: request.title.clone(),
            root_agent_id: request.agent.id.clone(),
            status: SessionStatus::Running,
            created_at,
            updated_at: created_at,
        };
        self.store.create_session(&session).await?;
        self.emit(
            &session_id,
            &request.agent.id,
            EventType::SessionStarted,
            json!({}),
        )
        .await?;
        self.save_message(
            &session_id,
            None,
            Some(request.agent.id.clone()),
            MessageRole::User,
            request.input.clone(),
            json!({}),
        )
        .await?;

        let provider = provider_for(&request.agent.model)?;
        let memory_context = if request.agent.memory.enabled {
            let memories = self
                .store
                .search_memories(Some(&request.agent.id), &request.input, 8)
                .await?;
            if !memories.is_empty() {
                self.emit(
                    &session_id,
                    &request.agent.id,
                    EventType::MemoryRead,
                    json!({"count": memories.len()}),
                )
                .await?;
            }
            render_memory_context(&memories)
        } else {
            None
        };

        let mut conversation = vec![
            ChatMessage {
                role: "system".to_string(),
                content: build_system_prompt(
                    &request.agent,
                    self.definitions_for_agent(&request.agent),
                    memory_context.as_deref(),
                ),
            },
            ChatMessage {
                role: "user".to_string(),
                content: request.input.clone(),
            },
        ];

        for iteration in 1..=request.agent.max_iterations {
            if self.store.get_session(&session_id).await?.status == SessionStatus::Cancelled {
                self.emit(
                    &session_id,
                    &request.agent.id,
                    EventType::RunCancelled,
                    json!({"iteration": iteration}),
                )
                .await?;
                return Err(Agent1Error::Runtime("run was cancelled".to_string()));
            }
            self.emit(
                &session_id,
                &request.agent.id,
                EventType::ModelCallStarted,
                json!({"iteration": iteration}),
            )
            .await?;
            let response = provider
                .chat_stream(ChatRequest {
                    model: request.agent.model.clone(),
                    messages: conversation.clone(),
                })
                .await?;
            if response.chunks.len() > 1 {
                for (index, chunk) in response.chunks.iter().enumerate() {
                    self.emit(
                        &session_id,
                        &request.agent.id,
                        EventType::ModelOutputDelta,
                        json!({"iteration": iteration, "chunk_index": index + 1, "content": chunk}),
                    )
                    .await?;
                }
            }
            self.emit(
                &session_id,
                &request.agent.id,
                EventType::ModelCallCompleted,
                json!({"iteration": iteration, "bytes": response.content.len()}),
            )
            .await?;
            self.save_message(
                &session_id,
                Some(request.agent.id.clone()),
                None,
                MessageRole::Assistant,
                response.content.clone(),
                json!({"iteration": iteration}),
            )
            .await?;

            match parse_model_response(&response.content) {
                ModelAction::Final(answer) => {
                    self.emit(
                        &session_id,
                        &request.agent.id,
                        EventType::FinalAnswer,
                        json!({"bytes": answer.len()}),
                    )
                    .await?;
                    self.store
                        .update_session_status(&session_id, SessionStatus::Completed)
                        .await?;
                    return Ok(RunAgentResult {
                        session_id,
                        final_answer: answer,
                    });
                }
                ModelAction::ToolCall(call) => {
                    let observation = self
                        .execute_tool_request(
                            &request.agent,
                            &session_id,
                            &request.workspace_root,
                            call.name,
                            call.input,
                        )
                        .await?;
                    let tool_message = format!(
                        "Tool result. Continue the task. Return JSON only.\n{}",
                        serde_json::to_string_pretty(&observation)
                            .unwrap_or_else(|_| "{}".to_string())
                    );
                    self.save_message(
                        &session_id,
                        None,
                        Some(request.agent.id.clone()),
                        MessageRole::Tool,
                        tool_message.clone(),
                        json!({}),
                    )
                    .await?;
                    conversation.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: response.content,
                    });
                    conversation.push(ChatMessage {
                        role: "user".to_string(),
                        content: tool_message,
                    });
                }
            }
        }

        self.store
            .update_session_status(&session_id, SessionStatus::Failed)
            .await?;
        let message = format!(
            "agent reached max_iterations ({}) before producing a final answer",
            request.agent.max_iterations
        );
        self.emit(
            &session_id,
            &request.agent.id,
            EventType::Error,
            json!({"message": message}),
        )
        .await?;
        Err(Agent1Error::Runtime(message))
    }

    async fn execute_tool_request(
        &self,
        agent: &Agent,
        session_id: &str,
        workspace_root: &PathBuf,
        tool_name: String,
        input: Value,
    ) -> Result<ToolResult> {
        if !agent
            .tools
            .iter()
            .any(|configured| configured == &tool_name)
        {
            return Err(Agent1Error::PermissionDenied(format!(
                "agent `{}` is not configured for tool `{tool_name}`",
                agent.id
            )));
        }
        let native_tool = self.tools.get(&tool_name);
        if native_tool.is_none() && runtime_tool_definition(&tool_name).is_none() {
            return Err(Agent1Error::ToolNotFound(tool_name.clone()));
        }
        let mode = agent
            .permissions
            .get(&tool_name)
            .copied()
            .unwrap_or_else(|| PermissionMode::default_for_tool(&tool_name));
        if mode == PermissionMode::Deny {
            self.emit(
                session_id,
                &agent.id,
                EventType::ToolApprovalDecided,
                json!({"tool": tool_name, "decision": "denied_by_policy"}),
            )
            .await?;
            return Err(Agent1Error::PermissionDenied(format!(
                "policy denies tool `{tool_name}`"
            )));
        }
        if mode == PermissionMode::Ask {
            let approval_id = new_id("approval");
            let approval_request = json!({
                "approval_id": approval_id,
                "tool_name": tool_name,
                "agent_id": agent.id,
                "input": input,
                "risk_level": risk_label(risk_for_tool(&tool_name)),
            });
            self.store
                .save_approval_request(&ApprovalRecord {
                    id: approval_id.clone(),
                    session_id: session_id.to_string(),
                    agent_id: agent.id.clone(),
                    request: approval_request.clone(),
                    decision: None,
                    decided_at: None,
                    created_at: now(),
                })
                .await?;
            self.emit(
                session_id,
                &agent.id,
                EventType::ToolApprovalRequested,
                approval_request,
            )
            .await?;
            let approved = self
                .approvals
                .approve(ApprovalRequest {
                    approval_id: approval_id.clone(),
                    session_id: session_id.to_string(),
                    agent_id: agent.id.clone(),
                    tool_name: tool_name.clone(),
                    input: input.clone(),
                    risk: risk_for_tool(&tool_name),
                })
                .await?;
            self.emit(
                session_id,
                &agent.id,
                EventType::ToolApprovalDecided,
                json!({"tool": tool_name, "approved": approved}),
            )
            .await?;
            self.store
                .update_approval_decision(
                    &approval_id,
                    if approved { "approved" } else { "denied" },
                )
                .await?;
            if !approved {
                let denied = ToolCallRecord {
                    id: new_id("tool"),
                    session_id: session_id.to_string(),
                    agent_id: agent.id.clone(),
                    tool_name,
                    input,
                    output: None,
                    status: ToolCallStatus::Denied,
                    error: Some("user denied tool call".to_string()),
                    started_at: now(),
                    finished_at: Some(now()),
                };
                self.store.save_tool_call(&denied).await?;
                return Err(Agent1Error::PermissionDenied(
                    "user denied tool call".to_string(),
                ));
            }
        }

        let started_at = now();
        let call_id = new_id("tool");
        self.emit(
            session_id,
            &agent.id,
            EventType::ToolCallStarted,
            json!({"tool": tool_name, "call_id": call_id}),
        )
        .await?;
        let result = if let Some(tool) = native_tool {
            tool.execute(
                    input.clone(),
                    ToolContext {
                        workspace_root: workspace_root.clone(),
                        agent_id: agent.id.clone(),
                        session_id: session_id.to_string(),
                    },
                )
                .await
        } else {
            self.execute_runtime_tool(agent, session_id, workspace_root, &tool_name, input.clone())
                .await
        };
        match result {
            Ok(result) => {
                let output = json!({"content": result.content, "metadata": result.metadata});
                let record = ToolCallRecord {
                    id: call_id.clone(),
                    session_id: session_id.to_string(),
                    agent_id: agent.id.clone(),
                    tool_name: tool_name.clone(),
                    input,
                    output: Some(output.clone()),
                    status: ToolCallStatus::Completed,
                    error: None,
                    started_at,
                    finished_at: Some(now()),
                };
                self.store.save_tool_call(&record).await?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::ToolCallCompleted,
                    json!({"tool": tool_name, "call_id": call_id}),
                )
                .await?;
                Ok(ToolResult {
                    content: output["content"].as_str().unwrap_or("").to_string(),
                    metadata: output["metadata"].clone(),
                })
            }
            Err(err) => {
                let record = ToolCallRecord {
                    id: call_id.clone(),
                    session_id: session_id.to_string(),
                    agent_id: agent.id.clone(),
                    tool_name: tool_name.clone(),
                    input,
                    output: None,
                    status: ToolCallStatus::Failed,
                    error: Some(err.to_string()),
                    started_at,
                    finished_at: Some(now()),
                };
                self.store.save_tool_call(&record).await?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::ToolCallFailed,
                    json!({"tool": tool_name, "call_id": call_id, "error": err.to_string()}),
                )
                .await?;
                Err(err)
            }
        }
    }

    fn definitions_for_agent(&self, agent: &Agent) -> Vec<ToolDefinition> {
        let mut definitions = self.tools.definitions_for(&agent.tools);
        for tool_name in &agent.tools {
            if self.tools.get(tool_name).is_none() {
                if let Some(definition) = runtime_tool_definition(tool_name) {
                    definitions.push(definition);
                }
            }
        }
        definitions
    }

    async fn execute_runtime_tool(
        &self,
        agent: &Agent,
        session_id: &str,
        workspace_root: &PathBuf,
        tool_name: &str,
        input: Value,
    ) -> Result<ToolResult> {
        match tool_name {
            "memory_search" => {
                let input: MemorySearchInput = serde_json::from_value(input)
                    .map_err(|err| Agent1Error::Config(format!("invalid memory_search input: {err}")))?;
                let memories = self
                    .store
                    .search_memories(Some(&agent.id), &input.query, input.limit.unwrap_or(8).min(50) as i64)
                    .await?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::MemoryRead,
                    json!({"count": memories.len(), "query": input.query}),
                )
                .await?;
                Ok(ToolResult {
                    content: serde_json::to_string_pretty(&memories)
                        .unwrap_or_else(|_| "[]".to_string()),
                    metadata: json!({"count": memories.len()}),
                })
            }
            "memory_write" => {
                let input: MemoryWriteInput = serde_json::from_value(input)
                    .map_err(|err| Agent1Error::Config(format!("invalid memory_write input: {err}")))?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::MemoryWriteRequested,
                    json!({"scope": input.scope, "tags": input.tags}),
                )
                .await?;
                let now = now();
                let item = MemoryItem {
                    id: new_id("mem"),
                    scope: input.scope.unwrap_or_else(|| "agent".to_string()),
                    agent_id: Some(agent.id.clone()),
                    content: input.content,
                    tags: input.tags.unwrap_or_default(),
                    embedding: None,
                    importance: input.importance.unwrap_or(0),
                    created_at: now,
                    updated_at: now,
                };
                self.store.write_memory(&item).await?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::MemoryWritten,
                    json!({"memory_id": item.id}),
                )
                .await?;
                Ok(ToolResult {
                    content: serde_json::to_string_pretty(&item).unwrap_or_else(|_| "{}".to_string()),
                    metadata: json!({"memory_id": item.id}),
                })
            }
            "agent_call" => {
                let input: AgentCallInput = serde_json::from_value(input)
                    .map_err(|err| Agent1Error::Config(format!("invalid agent_call input: {err}")))?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::AgentHandoffRequested,
                    json!({"from_agent_id": agent.id, "to_agent_id": input.agent_id, "task": input.task}),
                )
                .await?;
                let target_agent = self.store.get_agent(&input.agent_id).await?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::AgentHandoffStarted,
                    json!({"from_agent_id": agent.id, "to_agent_id": target_agent.id}),
                )
                .await?;
                let runtime = AgentRuntime::new(
                    self.store.clone(),
                    self.tools.clone(),
                    self.approvals.clone(),
                );
                let result = Box::pin(runtime.run(RunAgentRequest {
                    title: Some(format!("Delegated from {}", agent.id)),
                    agent: target_agent,
                    input: input.task,
                    workspace_root: workspace_root.clone(),
                }))
                .await?;
                self.emit(
                    session_id,
                    &agent.id,
                    EventType::AgentHandoffCompleted,
                    json!({"child_session_id": result.session_id}),
                )
                .await?;
                Ok(ToolResult {
                    content: result.final_answer,
                    metadata: json!({"child_session_id": result.session_id}),
                })
            }
            "mcp_call" => {
                let input: McpCallInput = serde_json::from_value(input)
                    .map_err(|err| Agent1Error::Config(format!("invalid mcp_call input: {err}")))?;
                let server = self.store.get_mcp_server(&input.server).await?;
                if !server.enabled {
                    return Err(Agent1Error::PermissionDenied(format!(
                        "MCP server `{}` is disabled",
                        server.name
                    )));
                }
                let value = call_mcp_tool(&server, &input.tool, input.input).await?;
                Ok(ToolResult {
                    content: serde_json::to_string_pretty(&value)
                        .unwrap_or_else(|_| value.to_string()),
                    metadata: json!({"server": server.name, "tool": input.tool}),
                })
            }
            _ => Err(Agent1Error::ToolNotFound(tool_name.to_string())),
        }
    }

    async fn save_message(
        &self,
        session_id: &str,
        from_agent_id: Option<String>,
        to_agent_id: Option<String>,
        role: MessageRole,
        content: String,
        metadata: Value,
    ) -> Result<()> {
        self.store
            .save_message(&Message {
                id: new_id("msg"),
                session_id: session_id.to_string(),
                from_agent_id,
                to_agent_id,
                role,
                content,
                metadata,
                created_at: now(),
            })
            .await
    }

    async fn emit(
        &self,
        session_id: &str,
        agent_id: &str,
        event_type: EventType,
        payload: Value,
    ) -> Result<()> {
        self.store
            .save_event(&RuntimeEvent {
                id: new_id("evt"),
                session_id: Some(session_id.to_string()),
                agent_id: Some(agent_id.to_string()),
                event_type,
                payload,
                created_at: now(),
            })
            .await
    }
}

fn risk_label(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
    }
}

fn build_system_prompt(
    agent: &Agent,
    tools: Vec<agent1_core::ToolDefinition>,
    memory_context: Option<&str>,
) -> String {
    let tools_json = serde_json::to_string_pretty(&tools).unwrap_or_else(|_| "[]".to_string());
    let memory_context = memory_context.unwrap_or("No relevant memory was loaded.");
    format!(
        r#"{system_prompt}

You are running inside Agent1, a local-first personal agent runtime.

Return only one JSON object per response. Do not wrap it in Markdown.

Use this final answer shape:
{{"final":"your answer"}}

Use this tool request shape when a tool is needed:
{{"tool_call":{{"name":"file_read","input":{{"path":"README.md"}}}}}}

Available tools:
{tools_json}

Relevant local memory:
{memory_context}

Respect local-first safety. Ask for tools only when they materially improve the answer.
"#,
        system_prompt = agent.system_prompt,
        tools_json = tools_json,
        memory_context = memory_context
    )
}

fn render_memory_context(memories: &[MemoryItem]) -> Option<String> {
    if memories.is_empty() {
        return None;
    }
    let lines = memories
        .iter()
        .map(|memory| {
            format!(
                "- {} [{}]: {}",
                memory.id,
                memory.scope,
                memory.content.replace('\n', " ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(lines)
}

pub fn runtime_tool_definition(tool_name: &str) -> Option<ToolDefinition> {
    let definition = match tool_name {
        "memory_search" => ToolDefinition {
            name: "memory_search".to_string(),
            description: "Search persisted local memory for this agent.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 50}
                }
            }),
        },
        "memory_write" => ToolDefinition {
            name: "memory_write".to_string(),
            description: "Persist a local memory item for future runs.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["content"],
                "properties": {
                    "content": {"type": "string"},
                    "scope": {"type": "string", "enum": ["agent", "global"]},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "importance": {"type": "integer"}
                }
            }),
        },
        "agent_call" => ToolDefinition {
            name: "agent_call".to_string(),
            description: "Delegate a task to another saved Agent1 agent and return its result."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["agent_id", "task"],
                "properties": {
                    "agent_id": {"type": "string"},
                    "task": {"type": "string"}
                }
            }),
        },
        "mcp_call" => ToolDefinition {
            name: "mcp_call".to_string(),
            description: "Call a tool exposed by an enabled stdio MCP server.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["server", "tool"],
                "properties": {
                    "server": {"type": "string"},
                    "tool": {"type": "string"},
                    "input": {"type": "object"}
                }
            }),
        },
        _ => return None,
    };
    Some(definition)
}

#[derive(Debug, Deserialize)]
struct MemorySearchInput {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct MemoryWriteInput {
    content: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    importance: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct AgentCallInput {
    agent_id: String,
    task: String,
}

#[derive(Debug, Deserialize)]
struct McpCallInput {
    server: String,
    tool: String,
    #[serde(default)]
    input: Value,
}

#[derive(Debug, Deserialize)]
struct ToolCallEnvelope {
    name: String,
    #[serde(default)]
    input: Value,
}

#[derive(Debug)]
enum ModelAction {
    Final(String),
    ToolCall(ToolCallEnvelope),
}

fn parse_model_response(content: &str) -> ModelAction {
    let trimmed = content.trim();
    let parsed: Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(_) => return ModelAction::Final(content.to_string()),
    };
    if let Some(final_answer) = parsed.get("final").and_then(Value::as_str) {
        return ModelAction::Final(final_answer.to_string());
    }
    if let Some(tool_call) = parsed.get("tool_call") {
        if let Ok(call) = serde_json::from_value::<ToolCallEnvelope>(tool_call.clone()) {
            return ModelAction::ToolCall(call);
        }
    }
    ModelAction::Final(content.to_string())
}

fn risk_for_tool(tool_name: &str) -> RiskLevel {
    match tool_name {
        "file_read" | "file_list" | "workspace_search" | "git_status" | "git_diff"
        | "memory_search" => RiskLevel::Low,
        "file_write" | "task_board" | "memory_write" | "agent_call" | "mcp_call" => {
            RiskLevel::Medium
        }
        "shell" => RiskLevel::High,
        _ => RiskLevel::High,
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

type McpReader = tokio::io::Lines<BufReader<ChildStdout>>;

struct McpSession {
    child: Child,
    stdin: ChildStdin,
    reader: McpReader,
    stderr_log: Arc<Mutex<String>>,
    initialized: bool,
    next_id: u64,
    last_used_at: std::time::Instant,
}

const MAX_MCP_SERVERS: usize = 10;

static MCP_SESSION_POOL: OnceLock<Mutex<HashMap<String, Arc<Mutex<McpSession>>>>> = OnceLock::new();

fn mcp_pool() -> &'static Mutex<HashMap<String, Arc<Mutex<McpSession>>>> {
    MCP_SESSION_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn call_mcp_tool(server: &McpServerConfig, tool_name: &str, input: Value) -> Result<Value> {
    let params = json!({
        "name": tool_name,
        "arguments": if input.is_null() { json!({}) } else { input }
    });
    call_mcp(server, "tools/call", Some(params)).await
}

pub async fn list_mcp_tools(server: &McpServerConfig) -> Result<Value> {
    call_mcp(server, "tools/list", Some(json!({}))).await
}

pub async fn check_mcp_server_health(server: &McpServerConfig) -> bool {
    let key = match mcp_server_key(server) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let pool = mcp_pool().lock().await;
    let Some(session_arc) = pool.get(&key) else {
        return false;
    };
    let mut session = match session_arc.try_lock() {
        Ok(s) => s,
        Err(_) => return false,
    };
    if session.child.try_wait().ok().flatten().is_some() {
        return false;
    }
    true
}

async fn call_mcp(server: &McpServerConfig, method: &str, params: Option<Value>) -> Result<Value> {
    if server.transport != "stdio" {
        return Err(Agent1Error::Config(format!(
            "unsupported MCP transport `{}`",
            server.transport
        )));
    }
    let key = mcp_server_key(server)?;
    for attempt in 0..2 {
        let session = get_or_start_mcp_session(server, &key).await?;
        let mut session = session.lock().await;
        session.last_used_at = std::time::Instant::now();
        if session.child.try_wait().ok().flatten().is_some() {
            drop(session);
            remove_mcp_session(&key).await;
            if attempt == 0 {
                continue;
            }
            return Err(Agent1Error::Runtime(format!(
                "MCP server `{}` exited before request",
                server.name
            )));
        }
        match call_mcp_on_session(&mut session, method, params.clone()).await {
            Ok(value) => return Ok(value),
            Err(err) => {
                let stderr_text = session.stderr_log.lock().await.clone();
                drop(session);
                remove_mcp_session(&key).await;
                if attempt == 0 {
                    continue;
                }
                return Err(attach_mcp_stderr(err, &stderr_text));
            }
        }
    }
    Err(Agent1Error::Runtime(format!(
        "failed to call MCP method `{method}`"
    )))
}

fn mcp_server_key(server: &McpServerConfig) -> Result<String> {
    let command = server
        .command
        .as_deref()
        .ok_or_else(|| Agent1Error::Config("stdio MCP server requires command".to_string()))?;
    let env = server
        .env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(";");
    Ok(format!(
        "{}|{}|{}|{}|{}",
        server.id,
        server.name,
        command,
        server.args.join(" "),
        env
    ))
}

async fn get_or_start_mcp_session(server: &McpServerConfig, key: &str) -> Result<Arc<Mutex<McpSession>>> {
    if let Some(existing) = mcp_pool().lock().await.get(key).cloned() {
        return Ok(existing);
    }
    let created = Arc::new(Mutex::new(start_mcp_session(server).await?));
    let mut pool = mcp_pool().lock().await;
    if let Some(existing) = pool.get(key).cloned() {
        return Ok(existing);
    }

    if pool.len() >= MAX_MCP_SERVERS {
        if let Some(first_key) = pool.keys().next().cloned() {
            pool.remove(&first_key);
        }
    }

    pool.insert(key.to_string(), created.clone());
    Ok(created)
}

async fn remove_mcp_session(key: &str) {
    mcp_pool().lock().await.remove(key);
}

pub async fn shutdown_mcp_pool() {
    let mut pool = mcp_pool().lock().await;
    for (_, session_arc) in pool.drain() {
        let mut session = session_arc.lock().await;
        let _ = session.child.kill().await;
    }
}

async fn start_mcp_session(server: &McpServerConfig) -> Result<McpSession> {
    let command = server
        .command
        .as_deref()
        .ok_or_else(|| Agent1Error::Config("stdio MCP server requires command".to_string()))?;
    let mut child = Command::new(command)
        .args(&server.args)
        .envs(&server.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Agent1Error::Runtime(format!("failed to start MCP server: {err}")))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| Agent1Error::Runtime("failed to open MCP stdin".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Agent1Error::Runtime("failed to open MCP stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Agent1Error::Runtime("failed to open MCP stderr".to_string()))?;
    let stderr_log = Arc::new(Mutex::new(String::new()));
    let stderr_log_task = stderr_log.clone();
    tokio::spawn(async move {
        let mut stderr_reader = BufReader::new(stderr).lines();
        loop {
            match stderr_reader.next_line().await {
                Ok(Some(line)) => {
                    let mut log = stderr_log_task.lock().await;
                    if !log.is_empty() {
                        log.push('\n');
                    }
                    log.push_str(&line);
                    if log.len() > 8_000 {
                        let keep_from = log.len().saturating_sub(8_000);
                        log.drain(0..keep_from);
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
    });
    Ok(McpSession {
        child,
        stdin,
        reader: BufReader::new(stdout).lines(),
        stderr_log,
        initialized: false,
        next_id: 1,
        last_used_at: std::time::Instant::now(),
    })
}

async fn call_mcp_on_session(
    session: &mut McpSession,
    method: &str,
    params: Option<Value>,
) -> Result<Value> {
    if !session.initialized {
        let init_id = session.next_id;
        session.next_id += 1;
        write_json_line(
            &mut session.stdin,
            &JsonRpcRequest {
                jsonrpc: "2.0",
                id: init_id,
                method: "initialize",
                params: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "agent1", "version": env!("CARGO_PKG_VERSION")}
                })),
            },
        )
        .await?;
        let _ = read_json_rpc_response(&mut session.reader, init_id).await?;
        session
            .stdin
            .write_all(br#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to write MCP initialized: {err}")))?;
        session
            .stdin
            .write_all(b"\n")
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to write MCP newline: {err}")))?;
        session.initialized = true;
    }

    let id = session.next_id;
    session.next_id += 1;
    write_json_line(
        &mut session.stdin,
        &JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        },
    )
    .await?;
    read_json_rpc_response(&mut session.reader, id).await
}

fn attach_mcp_stderr(error: Agent1Error, stderr: &str) -> Agent1Error {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        return error;
    }
    Agent1Error::Runtime(format!("{error}; MCP stderr: {stderr}"))
}

async fn write_json_line(
    stdin: &mut tokio::process::ChildStdin,
    request: &JsonRpcRequest<'_>,
) -> Result<()> {
    let line = serde_json::to_vec(request)
        .map_err(|err| Agent1Error::Runtime(format!("failed to encode MCP request: {err}")))?;
    stdin
        .write_all(&line)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to write MCP request: {err}")))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to write MCP newline: {err}")))?;
    Ok(())
}

async fn read_json_rpc_response(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: u64,
) -> Result<Value> {
    let deadline = Duration::from_secs(15);
    timeout(deadline, async {
        while let Some(line) = reader
            .next_line()
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to read MCP response: {err}")))?
        {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(&line).map_err(|err| {
                Agent1Error::InvalidModelResponse(format!("MCP response was not JSON: {err}"))
            })?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(Agent1Error::Runtime(format!("MCP error: {error}")));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
        Err(Agent1Error::Runtime(
            "MCP server closed stdout before response".to_string(),
        ))
    })
    .await
    .map_err(|_| Agent1Error::Runtime("MCP request timed out".to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Clone)]
    struct TestApprovals;

    #[async_trait]
    impl ApprovalDelegate for TestApprovals {
        async fn approve(&self, _request: ApprovalRequest) -> Result<bool> {
            Ok(true)
        }
    }

    #[test]
    fn parse_final_json() {
        match parse_model_response(r#"{"final":"done"}"#) {
            ModelAction::Final(answer) => assert_eq!(answer, "done"),
            _ => panic!("expected final"),
        }
    }

    #[test]
    fn parse_plain_text_as_final() {
        match parse_model_response("hello") {
            ModelAction::Final(answer) => assert_eq!(answer, "hello"),
            _ => panic!("expected final"),
        }
    }

    #[tokio::test]
    async fn runtime_completes_with_mock_final() {
        let store = test_store("mock-final").await;
        let runtime = AgentRuntime::new(store, ToolRegistry::with_defaults(), TestApprovals);
        let result = runtime
            .run(RunAgentRequest {
                agent: test_agent("final", vec![]),
                input: "finish".to_string(),
                title: Some("mock".to_string()),
                workspace_root: PathBuf::from("."),
            })
            .await
            .expect("mock final run should complete");
        assert_eq!(result.final_answer, "mock final from final");
    }

    #[tokio::test]
    async fn runtime_executes_tool_and_records_approval() {
        let store = test_store("mock-tool").await;
        let runtime = AgentRuntime::new(store.clone(), ToolRegistry::with_defaults(), TestApprovals);
        let result = runtime
            .run(RunAgentRequest {
                agent: test_agent("tool_file_list", vec!["file_list"]),
                input: "list files".to_string(),
                title: Some("mock tool".to_string()),
                workspace_root: PathBuf::from("."),
            })
            .await
            .expect("mock tool run should complete");
        assert_eq!(result.final_answer, "mock observed tool result");
        let approvals = store.recent_approvals(10).await.expect("approvals");
        assert_eq!(approvals.len(), 1);
        assert_eq!(approvals[0].decision.as_deref(), Some("approved"));
    }

    #[tokio::test]
    async fn runtime_marks_session_failed_when_max_iterations_hit() {
        let store = test_store("mock-failure").await;
        let runtime = AgentRuntime::new(store.clone(), ToolRegistry::with_defaults(), TestApprovals);
        let result = runtime
            .run(RunAgentRequest {
                agent: Agent {
                    max_iterations: 1,
                    ..test_agent("repeat_tool", vec!["file_list"])
                },
                input: "loop".to_string(),
                title: Some("mock failure".to_string()),
                workspace_root: PathBuf::from("."),
            })
            .await;
        assert!(result.is_err());
        let session = store
            .recent_sessions(1)
            .await
            .expect("sessions")
            .into_iter()
            .next()
            .expect("session saved");
        assert_eq!(session.status, SessionStatus::Failed);
    }

    #[tokio::test]
    async fn disabled_mcp_tool_cannot_be_called() {
        let store = test_store("mcp-disabled").await;
        let timestamp = now();
        store
            .save_mcp_server(&McpServerConfig {
                id: "disabled".to_string(),
                name: "disabled".to_string(),
                transport: "stdio".to_string(),
                command: Some("unused".to_string()),
                args: Vec::new(),
                env: Default::default(),
                enabled: false,
                created_at: timestamp,
                updated_at: timestamp,
            })
            .await
            .expect("save MCP server");
        let runtime = AgentRuntime::new(store, ToolRegistry::with_defaults(), TestApprovals);
        let result = runtime
            .run(RunAgentRequest {
                agent: test_agent("mcp_disabled", vec!["mcp_call"]),
                input: "call mcp".to_string(),
                title: Some("mcp disabled".to_string()),
                workspace_root: PathBuf::from("."),
            })
            .await;
        assert!(result.expect_err("disabled MCP should fail").to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn mcp_crash_is_logged_with_stderr() {
        let timestamp = now();
        let (command, args) = crash_server_command();
        let server = McpServerConfig {
            id: "crash".to_string(),
            name: "crash".to_string(),
            transport: "stdio".to_string(),
            command: Some(command),
            args,
            env: Default::default(),
            enabled: true,
            created_at: timestamp,
            updated_at: timestamp,
        };
        let error = list_mcp_tools(&server)
            .await
            .expect_err("crashing MCP server should fail")
            .to_string();
        assert!(error.contains("MCP stderr"));
        assert!(error.contains("mcp crashed"));
    }

    async fn test_store(name: &str) -> SqliteStore {
        let path = PathBuf::from("target").join(format!("agent1-runtime-{name}-{}.db", new_id("test")));
        SqliteStore::connect(path).await.expect("test sqlite store")
    }

    fn test_agent(model: &str, tools: Vec<&str>) -> Agent {
        Agent {
            id: format!("agent-{model}"),
            name: "Test Agent".to_string(),
            description: None,
            role: None,
            system_prompt: "Test agent.".to_string(),
            model: agent1_core::ModelConfig {
                provider: "mock".to_string(),
                model: model.to_string(),
                base_url: None,
                context_window: 8192,
                temperature: 0.0,
                top_p: None,
                max_tokens: None,
            },
            tools: tools.into_iter().map(ToString::to_string).collect(),
            memory: Default::default(),
            permissions: Default::default(),
            max_iterations: 4,
        }
    }

    #[cfg(windows)]
    fn crash_server_command() -> (String, Vec<String>) {
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Write-Error 'mcp crashed'; exit 1".to_string(),
            ],
        )
    }

    #[cfg(not(windows))]
    fn crash_server_command() -> (String, Vec<String>) {
        (
            "sh".to_string(),
            vec![
                "-lc".to_string(),
                "echo 'mcp crashed' 1>&2; exit 1".to_string(),
            ],
        )
    }
}
