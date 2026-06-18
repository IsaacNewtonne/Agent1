use crate::types::OrchestratorConfig;
use agent1_core::{
    Agent, Agent1Error, AgentRole, ExecutionStep, MemoryConfig, ModelConfig, PermissionMode, Result,
};
use agent1_db::SqliteStore;
use agent1_runtime::{AgentRuntime, ApprovalDelegate, ApprovalRequest, RunAgentRequest};
use agent1_tools::ToolRegistry;
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tokio::sync::Mutex;

pub struct TeamManager {
    store: SqliteStore,
    tools: ToolRegistry,
    active_agents: Mutex<HashMap<String, Agent>>,
}

impl TeamManager {
    pub fn new(store: SqliteStore, _config: OrchestratorConfig) -> Self {
        Self {
            store,
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
                    "memory_search".to_string(),
                    "memory_write".to_string(),
                    "agent_call".to_string(),
                    "mcp_call".to_string(),
                ]
            }
        };

        let model = role_model_config();

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

fn role_model_config() -> ModelConfig {
    #[cfg(test)]
    {
        ModelConfig {
            provider: "mock".to_string(),
            model: "final".to_string(),
            base_url: None,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        }
    }
    #[cfg(not(test))]
    {
        ModelConfig {
            provider: "ollama".to_string(),
            model: "llama3.1:8b".to_string(),
            base_url: None,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        }
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
