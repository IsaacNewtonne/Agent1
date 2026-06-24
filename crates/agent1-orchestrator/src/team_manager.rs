use crate::types::OrchestratorConfig;
use agent1_core::{
    Agent, Agent1Error, AgentRole, ExecutionStep, MemoryConfig, ModelConfig, ModelInfo,
    PermissionMode, Result,
};
use agent1_db::SqliteStore;
use agent1_models::provider_for;
use agent1_runtime::{AgentRuntime, ApprovalDelegate, ApprovalRequest, RunAgentRequest};
use agent1_tools::ToolRegistry;
use async_trait::async_trait;
use futures_util::future::join_all;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tokio::sync::Mutex;

pub struct TeamManager {
    store: SqliteStore,
    config: OrchestratorConfig,
    tools: ToolRegistry,
    active_agents: Mutex<HashMap<String, Agent>>,
}

impl TeamManager {
    pub fn new(store: SqliteStore, config: OrchestratorConfig) -> Self {
        Self {
            store,
            config,
            tools: ToolRegistry::with_defaults(),
            active_agents: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create_agent_for_role(
        &self,
        role: AgentRole,
        orchestration_id: &str,
    ) -> Result<Agent> {
        let agent_id = format!("{}_{}", orchestration_id.replace('-', "_"), role.as_str());

        let base_tools = match role {
            AgentRole::Worker | AgentRole::Builder => {
                vec![
                    "file_read".to_string(),
                    "file_list".to_string(),
                    "file_write".to_string(),
                    "workspace_search".to_string(),
                    "git_status".to_string(),
                    "git_diff".to_string(),
                    "verification_check".to_string(),
                    "task_board".to_string(),
                ]
            }
            AgentRole::Researcher => {
                vec![
                    "file_read".to_string(),
                    "file_list".to_string(),
                    "workspace_search".to_string(),
                    "memory_search".to_string(),
                ]
            }
            AgentRole::Critic => {
                vec![
                    "file_read".to_string(),
                    "file_list".to_string(),
                    "workspace_search".to_string(),
                    "git_status".to_string(),
                    "git_diff".to_string(),
                    "verification_check".to_string(),
                    "memory_search".to_string(),
                    "memory_write".to_string(),
                ]
            }
            AgentRole::Reporter => {
                vec![
                    "file_read".to_string(),
                    "file_list".to_string(),
                    "memory_search".to_string(),
                    "memory_write".to_string(),
                ]
            }
            AgentRole::Planner => {
                vec![
                    "file_read".to_string(),
                    "file_list".to_string(),
                    "workspace_search".to_string(),
                    "git_status".to_string(),
                    "verification_check".to_string(),
                    "memory_search".to_string(),
                    "memory_write".to_string(),
                    "agent_call".to_string(),
                ]
            }
            AgentRole::Orchestrator => {
                vec![
                    "file_read".to_string(),
                    "file_list".to_string(),
                    "workspace_search".to_string(),
                    "git_status".to_string(),
                    "verification_check".to_string(),
                    "memory_search".to_string(),
                    "memory_write".to_string(),
                    "agent_call".to_string(),
                    "mcp_call".to_string(),
                ]
            }
        };

        let model = self.select_model_for_role(role, &base_tools).await;

        let mut permissions = BTreeMap::new();
        for tool in &base_tools {
            permissions.insert(tool.clone(), PermissionMode::Ask);
        }

        if matches!(role, AgentRole::Worker | AgentRole::Builder) {
            permissions.insert("file_write".to_string(), PermissionMode::Allow);
        }

        if matches!(role, AgentRole::Reporter | AgentRole::Researcher) {
            permissions.insert("memory_write".to_string(), PermissionMode::Allow);
        }

        let agent = Agent {
            id: agent_id.clone(),
            name: format!("{:?} Agent", role),
            description: Some(format!(
                "{} for orchestration {}",
                orchestration_id,
                role.as_str()
            )),
            role: Some(role.as_str().to_string()),
            system_prompt: role.default_system_prompt().to_string(),
            model,
            tools: base_tools,
            memory: MemoryConfig { enabled: true },
            permissions,
            max_iterations: match role {
                AgentRole::Worker => 8,
                AgentRole::Builder => 12,
                AgentRole::Critic => 4,
                _ => 6,
            },
        };

        self.store.save_agent(&agent).await?;
        self.store
            .save_agent_card(&agent1_core::AgentCard {
                id: agent.id.clone(),
                name: agent.name.clone(),
                description: agent.description.clone(),
                skills: vec![agent1_core::AgentSkill {
                    name: role.as_str().to_string(),
                    description: format!("{:?} role agent", role),
                }],
                input_modes: vec!["text".to_string()],
                output_modes: vec!["text".to_string(), "markdown".to_string()],
                endpoint: format!("http://127.0.0.1:17371/api/agents/{}/tasks", agent.id),
            })
            .await?;

        let mut active = self.active_agents.lock().await;
        active.insert(agent_id, agent.clone());

        Ok(agent)
    }

    async fn select_model_for_role(&self, role: AgentRole, tools: &[String]) -> ModelConfig {
        #[cfg(test)]
        {
            let _ = tools;
            return self.config.model_routing.for_role(role).clone();
        }

        #[cfg(not(test))]
        {
            let preferred = self.config.model_routing.for_role(role).clone();
            if !preferred.provider.eq_ignore_ascii_case("codex")
                && model_is_explicitly_configured_for_role(role)
            {
                return preferred;
            }

            let mut candidates = available_subagent_models().await;
            candidates.sort_by(|left, right| {
                score_model_for_role(right, role, tools)
                    .cmp(&score_model_for_role(left, role, tools))
                    .then_with(|| {
                        provider_preference(&left.provider)
                            .cmp(&provider_preference(&right.provider))
                    })
            });

            if let Some(model) = candidates.into_iter().next() {
                return model_config_for(&model, role);
            }

            if preferred.provider.eq_ignore_ascii_case("codex") {
                fallback_subagent_model(role)
            } else {
                preferred
            }
        }
    }

    pub async fn get_agent(&self, agent_id: &str) -> Result<Agent> {
        let active = self.active_agents.lock().await;
        if let Some(agent) = active.get(agent_id) {
            return Ok(agent.clone());
        }

        self.store.get_agent(agent_id).await
    }

    pub async fn list_active_agents(&self) -> Vec<Agent> {
        let active = self.active_agents.lock().await;
        active.values().cloned().collect()
    }

    pub async fn run_step(
        &self,
        step: &ExecutionStep,
        workspace_root: PathBuf,
        auto_approve: bool,
    ) -> Result<String> {
        let agent_id = step
            .assigned_agent_id
            .as_ref()
            .ok_or_else(|| Agent1Error::Config("step has no assigned agent".to_string()))?;

        let agent = self.get_agent(agent_id).await?;

        let runtime = AgentRuntime::new(
            self.store.clone(),
            self.tools.clone(),
            CliApprovals { auto_approve },
        );

        let result = runtime
            .run(RunAgentRequest {
                title: Some(step.description.clone()),
                agent,
                input: step.description.clone(),
                project_id: None,
                workspace_root,
                session_id: None,
            })
            .await?;

        Ok(result.final_answer)
    }

    pub async fn terminate_agent(&self, agent_id: &str) -> Result<()> {
        let mut active = self.active_agents.lock().await;
        active.remove(agent_id);
        Ok(())
    }

    pub async fn terminate_all(&self) -> Result<()> {
        let mut active = self.active_agents.lock().await;
        active.clear();
        Ok(())
    }
}

#[cfg(not(test))]
fn model_is_explicitly_configured_for_role(role: AgentRole) -> bool {
    let role = role.as_str().to_ascii_uppercase();
    std::env::var(format!("AGENT1_MODEL_{role}")).is_ok()
        || std::env::var("AGENT1_MODEL_DEFAULT").is_ok()
}

#[cfg(not(test))]
async fn available_subagent_models() -> Vec<ModelInfo> {
    let configs = [
        probe_config("nvidia", None),
        probe_config("opencode", None),
        probe_config("ollama", Some("http://localhost:11434")),
        probe_config("openai_compatible", Some("http://localhost:8000/v1")),
    ];

    let probes = configs.into_iter().map(|config| async move {
        let provider = provider_for(&config).ok()?;
        provider.list_models(&config).await.ok()
    });

    join_all(probes)
        .await
        .into_iter()
        .flatten()
        .flatten()
        .filter(|model| !model.provider.eq_ignore_ascii_case("codex"))
        .collect()
}

#[cfg(not(test))]
fn probe_config(provider: &str, default_base_url: Option<&str>) -> ModelConfig {
    ModelConfig {
        provider: provider.to_string(),
        model: "unused".to_string(),
        base_url: provider_base_url(provider).or_else(|| default_base_url.map(ToString::to_string)),
        api_key: provider_api_key(provider),
        display_name: None,
        fallbacks: Vec::new(),
        context_window: 8192,
        temperature: 0.2,
        top_p: None,
        max_tokens: None,
    }
}

#[cfg(not(test))]
fn provider_base_url(provider: &str) -> Option<String> {
    let key = match provider {
        "nvidia" => "NVIDIA_BASE_URL",
        "ollama" => "OLLAMA_BASE_URL",
        "openai_compatible" => "AGENT1_OPENAI_COMPATIBLE_BASE_URL",
        _ => return None,
    };
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(test))]
fn provider_api_key(provider: &str) -> Option<String> {
    let key = match provider {
        "nvidia" => "NVIDIA_API_KEY",
        "openai_compatible" => "AGENT1_OPENAI_COMPATIBLE_API_KEY",
        _ => return None,
    };
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(test))]
fn model_config_for(model: &ModelInfo, role: AgentRole) -> ModelConfig {
    ModelConfig {
        provider: model.provider.clone(),
        model: model.name.clone(),
        base_url: provider_base_url(&model.provider),
        api_key: provider_api_key(&model.provider),
        display_name: Some(format!(
            "{} auto-selected for {}",
            model.name,
            role.as_str()
        )),
        fallbacks: Vec::new(),
        context_window: inferred_context_window(&model.name),
        temperature: role_temperature(role),
        top_p: None,
        max_tokens: None,
    }
}

#[cfg(not(test))]
fn fallback_subagent_model(role: AgentRole) -> ModelConfig {
    ModelConfig {
        provider: "ollama".to_string(),
        model: "llama3.1:8b".to_string(),
        base_url: provider_base_url("ollama")
            .or_else(|| Some("http://localhost:11434".to_string())),
        api_key: None,
        display_name: Some(format!("Ollama fallback for {}", role.as_str())),
        fallbacks: Vec::new(),
        context_window: 8192,
        temperature: role_temperature(role),
        top_p: None,
        max_tokens: None,
    }
}

#[cfg(not(test))]
fn score_model_for_role(model: &ModelInfo, role: AgentRole, tools: &[String]) -> i32 {
    let name = model.name.to_ascii_lowercase();
    let provider = model.provider.to_ascii_lowercase();
    let mut score = 0;

    score += match provider.as_str() {
        "nvidia" => 40,
        "opencode" => 32,
        "ollama" => 24,
        "openai_compatible" => 18,
        _ => 0,
    };

    if provider == "nvidia" {
        score += 18;
    }
    if name.contains("coder") || name.contains("code") || name.contains("dev") {
        score += 20;
    }
    if name.contains("qwen") || name.contains("deepseek") || name.contains("claude") {
        score += 12;
    }
    if name.contains("llama") || name.contains("nemotron") {
        score += 10;
    }
    if name.contains("mini") || name.contains("small") || name.contains("3b") {
        score -= 10;
    }
    if name.contains("70b")
        || name.contains("72b")
        || name.contains("90b")
        || name.contains("253b")
        || name.contains("405b")
    {
        score += 14;
    }

    match role {
        AgentRole::Planner | AgentRole::Critic | AgentRole::Orchestrator => {
            score += 10;
            if provider == "nvidia" {
                score += 10;
            }
        }
        AgentRole::Worker | AgentRole::Builder => {
            if tools.iter().any(|tool| tool == "file_write") {
                score += 8;
            }
            if name.contains("coder") || name.contains("code") {
                score += 16;
            }
        }
        AgentRole::Researcher | AgentRole::Reporter => {
            if name.contains("instruct") || name.contains("chat") || name.contains("llama") {
                score += 8;
            }
            if provider == "ollama" {
                score += 4;
            }
        }
    }

    score
}

#[cfg(not(test))]
fn provider_preference(provider: &str) -> i32 {
    match provider {
        "nvidia" => 0,
        "opencode" => 1,
        "ollama" => 2,
        "openai_compatible" => 3,
        _ => 9,
    }
}

#[cfg(not(test))]
fn inferred_context_window(model_name: &str) -> u32 {
    let name = model_name.to_ascii_lowercase();
    if name.contains("128k") || name.contains("131k") {
        131_072
    } else if name.contains("32k") {
        32_768
    } else if name.contains("16k") {
        16_384
    } else {
        8192
    }
}

#[cfg(not(test))]
fn role_temperature(role: AgentRole) -> f32 {
    match role {
        AgentRole::Critic => 0.1,
        AgentRole::Worker | AgentRole::Builder => 0.15,
        _ => 0.2,
    }
}

#[derive(Clone)]
struct CliApprovals {
    auto_approve: bool,
}

#[async_trait]
impl ApprovalDelegate for CliApprovals {
    async fn approve(&self, _request: ApprovalRequest) -> Result<bool> {
        Ok(self.auto_approve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn create_worker_agent() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let store = SqliteStore::connect(&db_path).await.unwrap();
        let manager = TeamManager::new(store, OrchestratorConfig::default());

        let agent = manager
            .create_agent_for_role(AgentRole::Worker, "test_orch")
            .await
            .expect("should create worker agent");

        assert!(agent.id.contains("worker"));
        assert!(agent.tools.contains(&"file_write".to_string()));
        assert!(agent.tools.contains(&"verification_check".to_string()));
        assert_eq!(
            agent.permissions.get("file_write"),
            Some(&PermissionMode::Allow)
        );
    }

    #[tokio::test]
    async fn create_critic_agent() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let store = SqliteStore::connect(&db_path).await.unwrap();
        let manager = TeamManager::new(store, OrchestratorConfig::default());

        let agent = manager
            .create_agent_for_role(AgentRole::Critic, "test_orch")
            .await
            .expect("should create critic agent");

        assert!(agent.id.contains("critic"));
        assert!(agent.tools.contains(&"memory_write".to_string()));
    }
}
