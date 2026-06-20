//! Agent1 Collaboration Engine
//!
//! Implements a hybrid system that automatically combines:
//! - Shared blackboard/project state
//! - Event-driven updates via broadcast channels
//! - Task routing/delegation based on collaboration mode
//! - Artifact-based collaboration tracking
//! - Tool-gated safe execution
//! - Supervisor orchestration by Agent1
//! - Live canvas/project sync
//! - Scoped external agent access

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use agent1_core::{
    new_id, now, Agent1Error, AuthorType, BlackboardEntry, CollabBehavior, CollabEvent,
    CollabEventType, CollabTask, CollabTaskStatus, CollaborationMode, ExternalAgent,
    ExternalAgentStatus, ExternalPermissions, InviteToken, Project, Result,
};
use agent1_db::SqliteStore;
use serde_json::{json, Value};
use tokio::sync::{broadcast, Mutex, RwLock};

/// The central collaboration engine.
///
/// This is the brain of the hybrid system — it automatically decides how to
/// coordinate agents based on the project's collaboration mode and current context.
pub struct CollaborationEngine {
    store: SqliteStore,
    /// In-memory blackboard cache for fast reads
    blackboard: Arc<RwLock<HashMap<String, HashMap<String, BlackboardEntry>>>>,
    /// Event bus for real-time updates to all listeners (UI, agents, gateway)
    event_tx: broadcast::Sender<CollabEvent>,
    /// Task queue per project
    task_queues: Arc<Mutex<HashMap<String, VecDeque<CollabTask>>>>,
    /// Connected agent presence tracking
    agent_presence: Arc<RwLock<HashMap<String, AgentPresence>>>,
}

/// Tracks an agent's presence in a project
#[derive(Debug, Clone)]
pub struct AgentPresence {
    pub agent_id: String,
    pub project_id: String,
    pub agent_type: AuthorType,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub active_tasks: u32,
}

impl CollaborationEngine {
    pub fn new(store: SqliteStore) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            store,
            blackboard: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            task_queues: Arc::new(Mutex::new(HashMap::new())),
            agent_presence: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to the collaboration event bus
    pub fn subscribe(&self) -> broadcast::Receiver<CollabEvent> {
        self.event_tx.subscribe()
    }

    // ─── Project Management ───

    pub async fn create_project(&self, name: String, mode: CollaborationMode) -> Result<Project> {
        let project = Project::new(name, mode);
        self.store.save_project(&project).await?;
        self.emit_event(
            &project.id,
            CollabEventType::ProjectCreated,
            None,
            json!({
                "project_name": project.name,
                "mode": project.collaboration_mode,
            }),
        )
        .await;
        Ok(project)
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Project> {
        self.store.get_project(project_id).await
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        self.store.list_projects().await
    }

    pub async fn update_project_mode(
        &self,
        project_id: &str,
        mode: CollaborationMode,
    ) -> Result<Project> {
        let mut project = self.store.get_project(project_id).await?;
        project.collaboration_mode = mode;
        project.updated_at = now();
        self.store.save_project(&project).await?;
        self.emit_event(
            project_id,
            CollabEventType::ModeChanged,
            None,
            json!({
                "new_mode": mode,
            }),
        )
        .await;
        Ok(project)
    }

    pub async fn add_agent_to_project(&self, project_id: &str, agent_id: &str) -> Result<Project> {
        let mut project = self.store.get_project(project_id).await?;
        if !project.local_agent_ids.contains(&agent_id.to_string()) {
            project.local_agent_ids.push(agent_id.to_string());
            project.updated_at = now();
            self.store.save_project(&project).await?;
            self.emit_event(
                project_id,
                CollabEventType::AgentJoined,
                Some(agent_id),
                json!({
                    "agent_type": "local",
                }),
            )
            .await;
        }
        Ok(project)
    }

    pub async fn remove_agent_from_project(
        &self,
        project_id: &str,
        agent_id: &str,
    ) -> Result<Project> {
        let mut project = self.store.get_project(project_id).await?;
        project.local_agent_ids.retain(|id| id != agent_id);
        project.updated_at = now();
        self.store.save_project(&project).await?;
        self.emit_event(
            project_id,
            CollabEventType::AgentLeft,
            Some(agent_id),
            json!({
                "agent_type": "local",
            }),
        )
        .await;
        Ok(project)
    }

    // ─── Automatic Behavior Decision ───

    /// In Automatic mode, Agent1 chooses behavior based on context.
    pub async fn decide_behavior(&self, project: &Project) -> CollabBehavior {
        let local_count = project.local_agent_ids.len();
        let external_count = self.count_connected_externals(&project.id).await;
        let active_tasks = self.count_active_tasks(&project.id).await;
        let has_risky = self.has_risky_pending(&project.id).await;

        let behavior = match project.collaboration_mode {
            CollaborationMode::Automatic => {
                if has_risky {
                    // Risky actions always go through approval regardless
                    CollabBehavior::SupervisedApproval
                } else if external_count > 0 && active_tasks > 2 {
                    // Many agents contributing at once → coordinate
                    CollabBehavior::CoordinatedParallel
                } else if local_count > 2 {
                    // Several local agents → delegate in parallel
                    CollabBehavior::DelegatedParallel
                } else if local_count > 0 {
                    // A few local agents → structured planning
                    CollabBehavior::PlanThenDelegate
                } else {
                    // Solo → execute directly
                    CollabBehavior::DirectExecution
                }
            }
            CollaborationMode::Structured => CollabBehavior::PlanThenDelegate,
            CollaborationMode::Fast => CollabBehavior::DelegatedParallel,
            CollaborationMode::Careful => CollabBehavior::SupervisedApproval,
            CollaborationMode::Enterprise => CollabBehavior::SupervisedApproval,
            CollaborationMode::Airgapped => CollabBehavior::PlanThenDelegate,
        };

        self.emit_event(
            &project.id,
            CollabEventType::BehaviorDecided,
            None,
            json!({
                "behavior": behavior,
                "local_agents": local_count,
                "external_agents": external_count,
                "active_tasks": active_tasks,
                "has_risky": has_risky,
            }),
        )
        .await;

        behavior
    }

    // ─── Blackboard (Shared Project State) ───

    pub async fn blackboard_read(&self, project_id: &str) -> Vec<BlackboardEntry> {
        let cache = self.blackboard.read().await;
        let cached: Vec<BlackboardEntry> = cache
            .get(project_id)
            .map(|entries| entries.values().cloned().collect())
            .unwrap_or_default();
        drop(cache);
        if !cached.is_empty() {
            return cached;
        }
        self.store
            .get_blackboard(project_id)
            .await
            .unwrap_or_default()
    }

    pub async fn blackboard_get(&self, project_id: &str, key: &str) -> Option<BlackboardEntry> {
        let cache = self.blackboard.read().await;
        let cached = cache
            .get(project_id)
            .and_then(|entries| entries.get(key).cloned());
        drop(cache);
        if cached.is_some() {
            return cached;
        }
        self.store
            .get_blackboard(project_id)
            .await
            .ok()
            .and_then(|entries| entries.into_iter().find(|entry| entry.key == key))
    }

    pub async fn blackboard_write(
        &self,
        project_id: &str,
        key: String,
        value: Value,
        author_id: String,
        author_type: AuthorType,
    ) -> Result<BlackboardEntry> {
        // Permission check for external authors
        if author_type == AuthorType::External {
            let ext = self.find_external_by_id(project_id, &author_id).await;
            if let Some(ext) = &ext {
                if !ext.permissions.can_write_blackboard {
                    return Err(Agent1Error::PermissionDenied(
                        "external agent not allowed to write blackboard".to_string(),
                    ));
                }
            }
        }

        let entry = BlackboardEntry::new(
            project_id.to_string(),
            key.clone(),
            value.clone(),
            author_id.clone(),
            author_type,
        );

        // Update in-memory cache
        {
            let mut cache = self.blackboard.write().await;
            cache
                .entry(project_id.to_string())
                .or_default()
                .insert(key.clone(), entry.clone());
        }

        // Persist to database
        self.store.save_blackboard_entry(&entry).await?;

        self.emit_event(
            project_id,
            CollabEventType::BlackboardUpdated,
            Some(&author_id),
            json!({
                "key": key,
                "author_type": author_type,
            }),
        )
        .await;

        Ok(entry)
    }

    // ─── Task Routing ───

    pub async fn submit_task(&self, project_id: &str, description: String) -> Result<CollabTask> {
        let task = CollabTask::new(project_id.to_string(), description.clone());
        self.store.save_collab_task(&task).await?;

        {
            let mut queues = self.task_queues.lock().await;
            queues
                .entry(project_id.to_string())
                .or_default()
                .push_back(task.clone());
        }

        self.emit_event(
            project_id,
            CollabEventType::TaskCreated,
            None,
            json!({
                "task_id": task.id,
                "description": description,
            }),
        )
        .await;

        Ok(task)
    }

    pub async fn assign_task(
        &self,
        task_id: &str,
        agent_id: &str,
        agent_type: AuthorType,
    ) -> Result<CollabTask> {
        let mut task = self.store.get_collab_task(task_id).await?;
        task.assigned_agent_id = Some(agent_id.to_string());
        task.assigned_agent_type = Some(agent_type);
        task.status = CollabTaskStatus::Assigned;
        self.store.save_collab_task(&task).await?;

        self.emit_event(
            &task.project_id,
            CollabEventType::TaskAssigned,
            Some(agent_id),
            json!({
                "task_id": task_id,
                "agent_type": agent_type,
            }),
        )
        .await;

        Ok(task)
    }

    pub async fn complete_task(&self, task_id: &str, output: String) -> Result<CollabTask> {
        let mut task = self.store.get_collab_task(task_id).await?;
        task.status = CollabTaskStatus::Completed;
        task.output = Some(output);
        task.completed_at = Some(now());
        self.store.save_collab_task(&task).await?;

        let agent_id = task.assigned_agent_id.as_deref();
        self.emit_event(
            &task.project_id,
            CollabEventType::TaskCompleted,
            agent_id,
            json!({
                "task_id": task_id,
            }),
        )
        .await;

        Ok(task)
    }

    pub async fn list_tasks(&self, project_id: &str) -> Result<Vec<CollabTask>> {
        self.store.list_collab_tasks(project_id).await
    }

    // ─── External Agent Management ───

    pub async fn generate_invite(
        &self,
        project_id: &str,
        permissions: ExternalPermissions,
        created_by: String,
    ) -> Result<InviteToken> {
        let project = self.store.get_project(project_id).await?;
        let invite = InviteToken::generate(&project, permissions, created_by);
        self.store.save_invite_token(&invite).await?;
        Ok(invite)
    }

    pub async fn accept_invite(&self, token: &str, agent_name: String) -> Result<ExternalAgent> {
        let invite = self.store.get_invite_token(token).await?;
        if invite.used_by.is_some() {
            return Err(Agent1Error::PermissionDenied(
                "invite already used".to_string(),
            ));
        }
        if let Some(expires) = invite.expires_at {
            if now() > expires {
                return Err(Agent1Error::PermissionDenied("invite expired".to_string()));
            }
        }

        let external = ExternalAgent::new(
            invite.project_id.clone(),
            agent_name.clone(),
            token.to_string(),
            invite.permissions.clone(),
        );
        self.store.save_external_agent(&external).await?;
        self.store.mark_invite_used(token, &agent_name).await?;

        self.emit_event(
            &invite.project_id,
            CollabEventType::ExternalConnected,
            Some(&external.id),
            json!({
                "agent_name": agent_name,
            }),
        )
        .await;

        Ok(external)
    }

    pub async fn list_externals(&self, project_id: &str) -> Result<Vec<ExternalAgent>> {
        self.store.list_external_agents(project_id).await
    }

    pub async fn update_external_heartbeat(&self, external_id: &str) -> Result<()> {
        self.store.update_external_heartbeat(external_id).await?;

        // Update presence
        let mut presence = self.agent_presence.write().await;
        if let Some(p) = presence.get_mut(external_id) {
            p.last_activity = now();
        }

        Ok(())
    }

    pub async fn disconnect_external(&self, project_id: &str, external_id: &str) -> Result<()> {
        self.store
            .update_external_status(external_id, ExternalAgentStatus::Disconnected)
            .await?;
        self.emit_event(
            project_id,
            CollabEventType::ExternalDisconnected,
            Some(external_id),
            json!({}),
        )
        .await;
        Ok(())
    }

    pub async fn revoke_external(&self, project_id: &str, external_id: &str) -> Result<()> {
        self.store
            .update_external_status(external_id, ExternalAgentStatus::Revoked)
            .await?;
        self.emit_event(
            project_id,
            CollabEventType::ExternalDisconnected,
            Some(external_id),
            json!({
                "reason": "revoked",
            }),
        )
        .await;
        Ok(())
    }

    // ─── Presence ───

    pub async fn register_presence(
        &self,
        agent_id: &str,
        project_id: &str,
        agent_type: AuthorType,
    ) {
        let now_ts = now();
        let mut presence = self.agent_presence.write().await;
        presence.insert(
            agent_id.to_string(),
            AgentPresence {
                agent_id: agent_id.to_string(),
                project_id: project_id.to_string(),
                agent_type,
                connected_at: now_ts,
                last_activity: now_ts,
                active_tasks: 0,
            },
        );
    }

    pub async fn get_presence(&self, project_id: &str) -> Vec<AgentPresence> {
        let presence = self.agent_presence.read().await;
        presence
            .values()
            .filter(|p| p.project_id == project_id)
            .cloned()
            .collect()
    }

    // ─── Project Summary (for UI) ───

    pub async fn project_summary(&self, project_id: &str) -> Result<ProjectSummary> {
        let project = self.store.get_project(project_id).await?;
        let externals = self.store.list_external_agents(project_id).await?;
        let tasks = self.store.list_collab_tasks(project_id).await?;
        let blackboard = self.blackboard_read(project_id).await;
        let behavior = self.decide_behavior(&project).await;

        let connected_external_count = externals
            .iter()
            .filter(|e| e.status == ExternalAgentStatus::Connected)
            .count();

        let active_task_count = tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    CollabTaskStatus::Assigned | CollabTaskStatus::InProgress
                )
            })
            .count();

        Ok(ProjectSummary {
            project,
            local_agent_count: 0, // Will be filled by the API layer with actual agent data
            external_agents: externals,
            connected_external_count,
            total_tasks: tasks.len(),
            active_task_count,
            completed_task_count: tasks
                .iter()
                .filter(|t| t.status == CollabTaskStatus::Completed)
                .count(),
            blackboard_entry_count: blackboard.len(),
            current_behavior: behavior,
        })
    }

    // ─── Internal Helpers ───

    async fn count_connected_externals(&self, project_id: &str) -> usize {
        self.store
            .list_external_agents(project_id)
            .await
            .map(|agents| {
                agents
                    .iter()
                    .filter(|a| a.status == ExternalAgentStatus::Connected)
                    .count()
            })
            .unwrap_or(0)
    }

    async fn count_active_tasks(&self, project_id: &str) -> usize {
        self.store
            .list_collab_tasks(project_id)
            .await
            .map(|tasks| {
                tasks
                    .iter()
                    .filter(|t| {
                        matches!(
                            t.status,
                            CollabTaskStatus::Assigned | CollabTaskStatus::InProgress
                        )
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    async fn has_risky_pending(&self, project_id: &str) -> bool {
        self.store
            .list_collab_tasks(project_id)
            .await
            .map(|tasks| {
                tasks.iter().any(|t| {
                    t.requires_approval
                        && matches!(
                            t.status,
                            CollabTaskStatus::Queued | CollabTaskStatus::Assigned
                        )
                })
            })
            .unwrap_or(false)
    }

    async fn find_external_by_id(&self, project_id: &str, agent_id: &str) -> Option<ExternalAgent> {
        self.store
            .list_external_agents(project_id)
            .await
            .ok()
            .and_then(|agents| agents.into_iter().find(|a| a.id == agent_id))
    }

    async fn emit_event(
        &self,
        project_id: &str,
        event_type: CollabEventType,
        agent_id: Option<&str>,
        payload: Value,
    ) {
        let event = CollabEvent {
            id: new_id("cevt"),
            project_id: project_id.to_string(),
            event_type,
            agent_id: agent_id.map(String::from),
            payload,
            created_at: now(),
        };
        // Best-effort broadcast — don't fail if no receivers
        let _ = self.event_tx.send(event.clone());
        // Also persist
        let _ = self.store.save_collab_event(&event).await;
    }
}

/// Summary of a project's collaboration state, used by the UI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectSummary {
    pub project: Project,
    pub local_agent_count: usize,
    pub external_agents: Vec<ExternalAgent>,
    pub connected_external_count: usize,
    pub total_tasks: usize,
    pub active_task_count: usize,
    pub completed_task_count: usize,
    pub blackboard_entry_count: usize,
    pub current_behavior: CollabBehavior,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_project_and_decide_behavior() {
        let temp = tempfile::TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let store = SqliteStore::connect(&db_path).await.unwrap();
        let engine = CollaborationEngine::new(store);

        let project = engine
            .create_project("Test".into(), CollaborationMode::Automatic)
            .await
            .unwrap();
        assert_eq!(project.name, "Test");

        let behavior = engine.decide_behavior(&project).await;
        // No agents → direct execution
        assert_eq!(behavior, CollabBehavior::DirectExecution);
    }

    #[tokio::test]
    async fn blackboard_read_write() {
        let temp = tempfile::TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let store = SqliteStore::connect(&db_path).await.unwrap();
        let engine = CollaborationEngine::new(store);

        let project = engine
            .create_project("BB Test".into(), CollaborationMode::Automatic)
            .await
            .unwrap();

        engine
            .blackboard_write(
                &project.id,
                "status".into(),
                json!("active"),
                "agent1".into(),
                AuthorType::Local,
            )
            .await
            .unwrap();

        let entry = engine.blackboard_get(&project.id, "status").await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, json!("active"));
    }
}
