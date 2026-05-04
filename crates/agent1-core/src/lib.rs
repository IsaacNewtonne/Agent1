use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub type AgentId = String;
pub type SessionId = String;
pub type MessageId = String;
pub type EventId = String;
pub type ToolCallId = String;
pub type MemoryId = String;
pub type ApprovalId = String;

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::now_v7().simple())
}

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn redact_secrets_text(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let has_secret_marker = [
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "authorization",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if has_secret_marker {
        return input
            .lines()
            .map(redact_secret_line)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut redacted = input
        .split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ");
    if input.contains('\n') {
        redacted = input
            .lines()
            .map(redact_secret_line)
            .collect::<Vec<_>>()
            .join("\n");
    }
    redacted
}

pub fn redact_secrets_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_secrets_text(text)),
        Value::Array(items) => Value::Array(items.iter().map(redact_secrets_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_secret_key(key) {
                        (key.clone(), Value::String("[REDACTED]".to_string()))
                    } else {
                        (key.clone(), redact_secrets_value(value))
                    }
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn redact_token(token: &str) -> String {
    let trimmed = token.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ',' || ch == ';');
    let secret_prefixes = ["sk-", "ghp_", "github_pat_", "xoxb-", "xoxp-", "AKIA"];
    if secret_prefixes
        .iter()
        .any(|prefix| trimmed.starts_with(prefix) && trimmed.len() > prefix.len() + 8)
    {
        token.replace(trimmed, "[REDACTED]")
    } else {
        token.to_string()
    }
}

fn redact_secret_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    for marker in [
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "authorization",
    ] {
        if lower.contains(marker) {
            for separator in ["=", ":", " "] {
                if let Some((left, _)) = line.split_once(separator) {
                    return format!("{left}{separator}[REDACTED]");
                }
            }
            return "[REDACTED]".to_string();
        }
    }
    line.split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "authorization",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

#[derive(Debug, Error)]
pub enum Agent1Error {
    #[error("agent `{0}` was not found")]
    AgentNotFound(String),
    #[error("tool `{0}` was not found")]
    ToolNotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("invalid model response: {0}")]
    InvalidModelResponse(String),
    #[error("path `{0}` escapes workspace boundary")]
    PathEscapesWorkspace(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, Agent1Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub system_prompt: String,
    pub model: ModelConfig,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub permissions: PermissionPolicy,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}

fn default_max_iterations() -> u32 {
    12
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_context_window")]
    pub context_window: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

fn default_context_window() -> u32 {
    8192
}

fn default_temperature() -> f32 {
    0.2
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default)]
    pub enabled: bool,
}

pub type PermissionPolicy = BTreeMap<String, PermissionMode>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Allow,
    Ask,
    Deny,
}

impl PermissionMode {
    pub fn default_for_tool(tool_name: &str) -> Self {
        match tool_name {
            "file_read" | "file_list" | "workspace_search" | "git_status" | "git_diff" => Self::Ask,
            "file_write" | "task_board" | "shell" | "memory_search" | "memory_write"
            | "agent_call" | "mcp_call" => Self::Ask,
            _ => Self::Deny,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub title: Option<String>,
    pub root_agent_id: AgentId,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub session_id: SessionId,
    pub from_agent_id: Option<AgentId>,
    pub to_agent_id: Option<AgentId>,
    pub role: MessageRole,
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub id: EventId,
    pub session_id: Option<SessionId>,
    pub agent_id: Option<AgentId>,
    pub event_type: EventType,
    #[serde(default)]
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SessionStarted,
    ModelCallStarted,
    ModelOutputDelta,
    ModelCallCompleted,
    ToolApprovalRequested,
    ToolApprovalDecided,
    ToolCallStarted,
    ToolCallCompleted,
    ToolCallFailed,
    MemoryRead,
    MemoryWriteRequested,
    MemoryWritten,
    AgentHandoffRequested,
    AgentHandoffStarted,
    AgentHandoffCompleted,
    RunCancelled,
    FinalAnswer,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: MemoryId,
    pub scope: String,
    pub agent_id: Option<AgentId>,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub embedding: Option<Value>,
    #[serde(default)]
    pub importance: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub id: AgentId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub skills: Vec<AgentSkill>,
    #[serde(default)]
    pub input_modes: Vec<String>,
    #[serde(default)]
    pub output_modes: Vec<String>,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: ApprovalId,
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub request: Value,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: ToolCallId,
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub tool_name: String,
    pub input: Value,
    pub output: Option<Value>,
    pub status: ToolCallStatus,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    Approved,
    Denied,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: ModelConfig,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamResponse {
    pub content: String,
    #[serde(default)]
    pub chunks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub provider: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_common_secret_shapes() {
        assert_eq!(
            redact_secrets_text("api_key=sk-1234567890abcdef"),
            "api_key=[REDACTED]"
        );
        assert_eq!(
            redact_secrets_text("token: ghp_1234567890abcdef"),
            "token:[REDACTED]"
        );
        let value = redact_secrets_value(&json!({
            "password": "open-sesame",
            "nested": {"authorization": "Bearer secret"}
        }));
        assert_eq!(value["password"], "[REDACTED]");
        assert_eq!(value["nested"]["authorization"], "[REDACTED]");
    }

    #[test]
    fn network_tools_are_denied_by_default() {
        assert_eq!(
            PermissionMode::default_for_tool("network_request"),
            PermissionMode::Deny
        );
    }

    #[test]
    fn permission_policy_is_defined() {
        let _policy = PermissionPolicy::default();
    }

    #[test]
    fn permission_mode_allowed_constants() {
        assert!(matches!(PermissionMode::Allow, PermissionMode::Allow));
        assert!(matches!(PermissionMode::Deny, PermissionMode::Deny));
        assert!(matches!(PermissionMode::Ask, PermissionMode::Ask));
    }
}
