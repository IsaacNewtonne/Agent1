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
pub type PlanId = String;
pub type StepId = String;
pub type OrchestrationId = String;
pub type EscalationId = String;
pub type SuggestionId = String;

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
            "file_read" | "file_list" | "workspace_search" | "git_status" | "git_diff"
            | "verification_check" => Self::Ask,
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
    #[serde(default)]
    pub project_id: Option<ProjectId>,
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
    OrchestrationStarted,
    PlanCreated,
    StepStarted,
    StepCompleted,
    StepFailed,
    EscalationCreated,
    EscalationResolved,
    AgentCreated,
    AgentTerminated,
    SuggestionCreated,
    SuggestionAccepted,
    SuggestionDismissed,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionType {
    FollowUp,
    Improvement,
    Routine,
    Contextual,
}

impl std::fmt::Display for SuggestionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuggestionType::FollowUp => write!(f, "follow_up"),
            SuggestionType::Improvement => write!(f, "improvement"),
            SuggestionType::Routine => write!(f, "routine"),
            SuggestionType::Contextual => write!(f, "contextual"),
        }
    }
}

impl std::str::FromStr for SuggestionType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "follow_up" => Ok(SuggestionType::FollowUp),
            "improvement" => Ok(SuggestionType::Improvement),
            "routine" => Ok(SuggestionType::Routine),
            "contextual" => Ok(SuggestionType::Contextual),
            other => Err(format!("unknown suggestion type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Dismissed,
    Expired,
}

impl std::fmt::Display for SuggestionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuggestionStatus::Pending => write!(f, "pending"),
            SuggestionStatus::Accepted => write!(f, "accepted"),
            SuggestionStatus::Dismissed => write!(f, "dismissed"),
            SuggestionStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for SuggestionStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(SuggestionStatus::Pending),
            "accepted" => Ok(SuggestionStatus::Accepted),
            "dismissed" => Ok(SuggestionStatus::Dismissed),
            "expired" => Ok(SuggestionStatus::Expired),
            other => Err(format!("unknown suggestion status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: SuggestionId,
    pub suggestion_type: SuggestionType,
    pub content: String,
    pub trigger_context: String,
    pub related_memory_id: Option<MemoryId>,
    pub status: SuggestionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub dismissed_at: Option<DateTime<Utc>>,
}

impl Suggestion {
    pub fn new(
        suggestion_type: SuggestionType,
        content: String,
        trigger_context: String,
        related_memory_id: Option<MemoryId>,
    ) -> Self {
        let now = now();
        Self {
            id: new_id("sug"),
            suggestion_type,
            content,
            trigger_context,
            related_memory_id,
            status: SuggestionStatus::Pending,
            created_at: now,
            updated_at: now,
            accepted_at: None,
            dismissed_at: None,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Orchestrator,
    Planner,
    Worker,
    Critic,
    Researcher,
    Builder,
    Reporter,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Orchestrator => "orchestrator",
            AgentRole::Planner => "planner",
            AgentRole::Worker => "worker",
            AgentRole::Critic => "critic",
            AgentRole::Researcher => "researcher",
            AgentRole::Builder => "builder",
            AgentRole::Reporter => "reporter",
        }
    }

    pub fn default_system_prompt(&self) -> &'static str {
        match self {
            AgentRole::Orchestrator => {
                "You are Agent1, the central orchestrator. Your role is to receive high-level objectives from the user, decompose them into execution plans, create and manage specialized agents, coordinate their work, verify completion, preserve durable lessons in memory, and report progress back. You make autonomous decisions about how to achieve goals while escalating security-sensitive actions to the user."
            }
            AgentRole::Planner => {
                "You are a Planner agent. Your role is to analyze objectives and create detailed execution plans. Break complex goals into ordered steps, identify dependencies, determine what specialized agents are needed for each step, define verification gates, and anticipate potential issues. Be thorough and consider edge cases."
            }
            AgentRole::Worker => {
                "You are a Worker agent. Your role is to execute assigned tasks according to specifications. Follow instructions precisely, inspect local context before acting, use available tools to accomplish your assigned step, verify code or configuration changes before finalizing, and report results clearly. If you encounter blockers, explain them and suggest alternatives."
            }
            AgentRole::Critic => {
                "You are a Critic agent. Your role is to review and quality-check the work of other agents. Evaluate outputs for correctness, completeness, safety, evidence quality, verification coverage, and alignment with requirements. Identify gaps, suggest improvements, and approve or reject work. Be thorough and constructive."
            }
            AgentRole::Researcher => {
                "You are a Researcher agent. Your role is to gather information, analyze data, and provide factual findings to support planning and execution. Search for relevant information, summarize findings, and identify knowledge gaps."
            }
            AgentRole::Builder => {
                "You are a Builder agent. Your role is to create artifacts — code, documents, infrastructure, or other tangible outputs. Follow specifications precisely, write high-quality work, and iterate based on feedback."
            }
            AgentRole::Reporter => {
                "You are a Reporter agent. Your role is to compile findings, progress updates, and final summaries. Synthesize information from multiple sources into clear, actionable reports for users and other agents."
            }
        }
    }
}

impl OrchestrationSession {
    pub fn new(objective: String) -> Self {
        let now = now();
        Self {
            id: new_id("orch"),
            objective,
            plan_id: None,
            status: OrchestrationStatus::Received,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}

impl ExecutionPlan {
    pub fn new(orchestration_id: OrchestrationId, objective: String, raw_goal: String) -> Self {
        Self {
            id: new_id("plan"),
            orchestration_id,
            objective,
            raw_goal,
            status: PlanStatus::Draft,
            created_at: now(),
            completed_at: None,
        }
    }
}

impl ExecutionStep {
    pub fn new(
        plan_id: PlanId,
        description: String,
        step_order: usize,
        dependencies: Vec<StepId>,
    ) -> Self {
        Self {
            id: new_id("step"),
            plan_id,
            description,
            step_order,
            assigned_agent_id: None,
            assigned_role: None,
            dependencies,
            status: StepStatus::Pending,
            output: None,
            review_notes: None,
            created_at: now(),
            started_at: None,
            completed_at: None,
            sub_plan_id: None,
        }
    }

    pub fn assign(&mut self, agent_id: AgentId, role: AgentRole) {
        self.assigned_agent_id = Some(agent_id);
        self.assigned_role = Some(role);
    }

    pub fn start(&mut self) {
        self.status = StepStatus::InProgress;
        self.started_at = Some(now());
    }

    pub fn complete(&mut self, output: String) {
        self.status = StepStatus::Completed;
        self.output = Some(output);
        self.completed_at = Some(now());
    }

    pub fn fail(&mut self, error: String) {
        self.status = StepStatus::Failed;
        self.output = Some(error);
        self.completed_at = Some(now());
    }
}

impl EscalationRecord {
    pub fn new(
        orchestration_id: OrchestrationId,
        step_id: Option<StepId>,
        escalation_type: EscalationType,
        description: String,
        payload: Value,
    ) -> Self {
        Self {
            id: new_id("esc"),
            orchestration_id,
            step_id,
            escalation_type,
            description,
            payload,
            status: EscalationStatus::Pending,
            response: None,
            created_at: now(),
            resolved_at: None,
        }
    }

    pub fn resolve(&mut self, response: String) {
        self.status = EscalationStatus::Resolved;
        self.response = Some(response);
        self.resolved_at = Some(now());
    }

    pub fn decline(&mut self, reason: String) {
        self.status = EscalationStatus::Declined;
        self.response = Some(reason);
        self.resolved_at = Some(now());
    }
}

impl OrchestrationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrchestrationStatus::Received => "received",
            OrchestrationStatus::Planning => "planning",
            OrchestrationStatus::Executing => "executing",
            OrchestrationStatus::WaitingApproval => "waiting_approval",
            OrchestrationStatus::Completed => "completed",
            OrchestrationStatus::Failed => "failed",
            OrchestrationStatus::Cancelled => "cancelled",
        }
    }
}

impl StepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepStatus::Pending => "pending",
            StepStatus::InProgress => "in_progress",
            StepStatus::Completed => "completed",
            StepStatus::Blocked => "blocked",
            StepStatus::NeedsReview => "needs_review",
            StepStatus::Failed => "failed",
        }
    }
}

pub fn check_escalation_triggers(content: &str) -> Option<(EscalationType, String)> {
    let content_lower = content.to_lowercase();

    let security_triggers = [
        "api_key",
        "apikey",
        "secret",
        "password",
        "token",
        "authorization",
    ];
    if security_triggers.iter().any(|t| content_lower.contains(t)) {
        return Some((
            EscalationType::Security,
            "Operation involves security-sensitive data".to_string(),
        ));
    }

    let finance_triggers = [
        "payment",
        "billing",
        "purchase",
        "subscription",
        "invoice",
        "refund",
    ];
    if finance_triggers.iter().any(|t| content_lower.contains(t)) {
        return Some((
            EscalationType::Finance,
            "Operation involves financial transaction".to_string(),
        ));
    }

    let access_triggers = [
        "oauth",
        "connect_account",
        "authentication",
        "login",
        "connect to my",
        "account access",
    ];
    if access_triggers.iter().any(|t| content_lower.contains(t)) {
        return Some((
            EscalationType::Access,
            "Operation requires account access".to_string(),
        ));
    }

    let identity_triggers = ["email", "phone", "send_sms", "send_email"];
    if identity_triggers.iter().any(|t| content_lower.contains(t)) {
        return Some((
            EscalationType::Identity,
            "Operation involves personal identity data".to_string(),
        ));
    }

    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub id: StepId,
    pub plan_id: PlanId,
    pub description: String,
    pub step_order: usize,
    pub assigned_agent_id: Option<AgentId>,
    pub assigned_role: Option<AgentRole>,
    pub dependencies: Vec<StepId>,
    pub status: StepStatus,
    pub output: Option<String>,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_plan_id: Option<PlanId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: PlanId,
    pub orchestration_id: OrchestrationId,
    pub objective: String,
    pub raw_goal: String,
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    Planned,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
    NeedsReview,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationSession {
    pub id: OrchestrationId,
    pub objective: String,
    pub plan_id: Option<PlanId>,
    pub status: OrchestrationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationStatus {
    Received,
    Planning,
    Executing,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRecord {
    pub id: EscalationId,
    pub orchestration_id: OrchestrationId,
    pub step_id: Option<StepId>,
    pub escalation_type: EscalationType,
    pub description: String,
    pub payload: Value,
    pub status: EscalationStatus,
    pub response: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationType {
    Security,
    Finance,
    Access,
    Identity,
    Approval,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationStatus {
    Pending,
    Resolved,
    Declined,
}

// ─── Hybrid Collaboration Workspace Types ───

pub type ProjectId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub collaboration_mode: CollaborationMode,
    #[serde(default)]
    pub local_agent_ids: Vec<AgentId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn new(name: String, mode: CollaborationMode) -> Self {
        let now = now();
        Self {
            id: new_id("proj"),
            name,
            description: None,
            collaboration_mode: mode,
            local_agent_ids: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationMode {
    Automatic,
    Structured,
    Fast,
    Careful,
    Enterprise,
    Airgapped,
}

impl Default for CollaborationMode {
    fn default() -> Self {
        Self::Automatic
    }
}

impl CollaborationMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Structured => "Structured",
            Self::Fast => "Fast",
            Self::Careful => "Careful",
            Self::Enterprise => "Enterprise",
            Self::Airgapped => "Airgapped",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Automatic => "Agent1 decides behavior based on context",
            Self::Structured => "Explicit plan → delegate → review cycle",
            Self::Fast => "Minimal oversight, parallel execution",
            Self::Careful => "Every action needs approval",
            Self::Enterprise => "Audit-heavy execution with external access controls",
            Self::Airgapped => "Local-only execution with external systems disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorType {
    Local,
    External,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub id: String,
    pub project_id: ProjectId,
    pub key: String,
    pub value: Value,
    pub author_agent_id: String,
    pub author_type: AuthorType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BlackboardEntry {
    pub fn new(
        project_id: ProjectId,
        key: String,
        value: Value,
        author_id: String,
        author_type: AuthorType,
    ) -> Self {
        let now = now();
        Self {
            id: new_id("bb"),
            project_id,
            key,
            value,
            author_agent_id: author_id,
            author_type,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPermissions {
    #[serde(default = "default_true")]
    pub can_read_blackboard: bool,
    #[serde(default)]
    pub can_write_blackboard: bool,
    #[serde(default)]
    pub can_create_artifacts: bool,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub can_delegate_tasks: bool,
    #[serde(default = "default_max_tasks")]
    pub max_concurrent_tasks: u32,
}

fn default_true() -> bool {
    true
}
fn default_max_tasks() -> u32 {
    2
}

impl Default for ExternalPermissions {
    fn default() -> Self {
        Self {
            can_read_blackboard: true,
            can_write_blackboard: false,
            can_create_artifacts: false,
            allowed_tools: Vec::new(),
            can_delegate_tasks: false,
            max_concurrent_tasks: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAgentStatus {
    Invited,
    Connected,
    Disconnected,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAgent {
    pub id: String,
    pub project_id: ProjectId,
    pub name: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    pub invite_token: String,
    pub capabilities: Vec<String>,
    pub permissions: ExternalPermissions,
    pub status: ExternalAgentStatus,
    #[serde(default)]
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl ExternalAgent {
    pub fn new(
        project_id: ProjectId,
        name: String,
        token: String,
        permissions: ExternalPermissions,
    ) -> Self {
        Self {
            id: new_id("ext"),
            project_id,
            name,
            endpoint: None,
            invite_token: token,
            capabilities: Vec::new(),
            permissions,
            status: ExternalAgentStatus::Invited,
            last_heartbeat: None,
            created_at: now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteToken {
    pub token: String,
    pub project_id: ProjectId,
    pub project_name: String,
    pub permissions: ExternalPermissions,
    pub created_by: String,
    #[serde(default)]
    pub gateway_url: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub used_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl InviteToken {
    pub fn generate(
        project: &Project,
        permissions: ExternalPermissions,
        created_by: String,
    ) -> Self {
        let token = format!("inv_{}", Uuid::now_v7().simple());
        Self {
            token,
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            permissions,
            created_by,
            gateway_url: None,
            expires_at: None,
            used_by: None,
            created_at: now(),
        }
    }

    /// Export as a shareable JSON string
    pub fn to_invite_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollabTaskStatus {
    Queued,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabTask {
    pub id: String,
    pub project_id: ProjectId,
    pub description: String,
    pub assigned_agent_id: Option<String>,
    pub assigned_agent_type: Option<AuthorType>,
    pub status: CollabTaskStatus,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub requires_approval: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
}

impl CollabTask {
    pub fn new(project_id: ProjectId, description: String) -> Self {
        Self {
            id: new_id("ctask"),
            project_id,
            description,
            assigned_agent_id: None,
            assigned_agent_type: None,
            status: CollabTaskStatus::Queued,
            output: None,
            requires_approval: false,
            created_at: now(),
            completed_at: None,
        }
    }
}

/// Describes the behavior the collaboration engine should adopt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollabBehavior {
    /// Agent1 executes directly, no delegation
    DirectExecution,
    /// Create plan, delegate steps, review results
    PlanThenDelegate,
    /// Delegate to multiple agents in parallel
    DelegatedParallel,
    /// Coordinate local + external agents in parallel
    CoordinatedParallel,
    /// Every action goes through approval
    SupervisedApproval,
}

/// Events broadcast through the collaboration event bus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabEvent {
    pub id: String,
    pub project_id: ProjectId,
    pub event_type: CollabEventType,
    pub agent_id: Option<String>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollabEventType {
    ProjectCreated,
    ProjectUpdated,
    AgentJoined,
    AgentLeft,
    BlackboardUpdated,
    TaskCreated,
    TaskAssigned,
    TaskCompleted,
    TaskFailed,
    ExternalConnected,
    ExternalDisconnected,
    ExternalHeartbeat,
    ContributionReceived,
    ArtifactCreated,
    ModeChanged,
    BehaviorDecided,
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
