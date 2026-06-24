pub use agent1_core::{
    new_id, now, Agent1Error, AgentId, AgentRole, EscalationId, EscalationRecord, EscalationStatus,
    EscalationType, ExecutionPlan, ExecutionStep, ModelConfig, OrchestrationId,
    OrchestrationSession, OrchestrationStatus, PlanId, PlanStatus, Result, StepId, StepStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_concurrent_agents: usize,
    pub max_plan_depth: usize,
    pub auto_review: bool,
    pub review_threshold: usize,
    pub model_routing: ModelRoutingConfig,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 4,
            max_plan_depth: 20,
            auto_review: true,
            review_threshold: 2,
            model_routing: ModelRoutingConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelRoutingConfig {
    pub planner: ModelConfig,
    pub worker: ModelConfig,
    pub critic: ModelConfig,
    pub researcher: ModelConfig,
    pub builder: ModelConfig,
    pub reporter: ModelConfig,
    pub orchestrator: ModelConfig,
}

impl ModelRoutingConfig {
    pub fn for_role(&self, role: AgentRole) -> &ModelConfig {
        match role {
            AgentRole::Planner => &self.planner,
            AgentRole::Worker => &self.worker,
            AgentRole::Critic => &self.critic,
            AgentRole::Researcher => &self.researcher,
            AgentRole::Builder => &self.builder,
            AgentRole::Reporter => &self.reporter,
            AgentRole::Orchestrator => &self.orchestrator,
        }
    }
}

impl Default for ModelRoutingConfig {
    fn default() -> Self {
        Self {
            planner: routed_model("PLANNER", 0.2),
            worker: routed_model("WORKER", 0.15),
            critic: routed_model("CRITIC", 0.1),
            researcher: routed_model("RESEARCHER", 0.2),
            builder: routed_model("BUILDER", 0.15),
            reporter: routed_model("REPORTER", 0.2),
            orchestrator: routed_model("ORCHESTRATOR", 0.2),
        }
    }
}

fn routed_model(role: &str, temperature: f32) -> ModelConfig {
    #[cfg(test)]
    {
        let _ = (role, temperature);
        ModelConfig {
            provider: "mock".to_string(),
            model: "final".to_string(),
            base_url: None,
            api_key: None,
            display_name: None,
            fallbacks: Vec::new(),
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        }
    }
    #[cfg(not(test))]
    {
        let provider = std::env::var(format!("AGENT1_MODEL_{role}_PROVIDER"))
            .or_else(|_| std::env::var("AGENT1_MODEL_DEFAULT_PROVIDER"))
            .unwrap_or_else(|_| "ollama".to_string());
        let model = std::env::var(format!("AGENT1_MODEL_{role}"))
            .or_else(|_| std::env::var("AGENT1_MODEL_DEFAULT"))
            .unwrap_or_else(|_| "llama3.1:8b".to_string());
        let base_url = std::env::var(format!("AGENT1_MODEL_{role}_BASE_URL"))
            .or_else(|_| std::env::var("AGENT1_MODEL_DEFAULT_BASE_URL"))
            .ok()
            .filter(|value| !value.trim().is_empty());
        let context_window = std::env::var(format!("AGENT1_MODEL_{role}_CONTEXT"))
            .or_else(|_| std::env::var("AGENT1_MODEL_DEFAULT_CONTEXT"))
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(8192);

        ModelConfig {
            provider,
            model,
            base_url,
            api_key: None,
            display_name: None,
            fallbacks: Vec::new(),
            context_window,
            temperature,
            top_p: None,
            max_tokens: None,
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
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub sub_steps: HashMap<PlanId, Vec<ExecutionStep>>,
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
