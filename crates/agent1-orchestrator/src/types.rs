pub use agent1_core::{
    new_id, now, Agent1Error, AgentId, AgentRole, EscalationId, EscalationRecord, EscalationStatus,
    EscalationType, ExecutionPlan, ExecutionStep, OrchestrationId, OrchestrationSession,
    OrchestrationStatus, PlanId, PlanStatus, Result, StepId, StepStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_concurrent_agents: usize,
    pub max_plan_depth: usize,
    pub auto_review: bool,
    pub review_threshold: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 4,
            max_plan_depth: 20,
            auto_review: true,
            review_threshold: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrateRequest {
    pub objective: String,
    pub workspace_root: Option<String>,
    pub auto_approve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrateResponse {
    pub orchestration_id: OrchestrationId,
    pub plan_id: PlanId,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepUpdate {
    pub step_id: StepId,
    pub status: StepStatus,
    pub output: Option<String>,
    pub review_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanView {
    pub plan: ExecutionPlan,
    pub steps: Vec<ExecutionStep>,
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
