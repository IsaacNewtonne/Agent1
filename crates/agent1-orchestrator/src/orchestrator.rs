use crate::{
    escalation::EscalationManager,
    goal_decomposer::GoalDecomposer,
    progress_tracker::ProgressTracker,
    team_manager::TeamManager,
    types::{
        check_escalation_triggers, OrchestrateRequest, OrchestrateResponse, OrchestratorConfig,
    },
};
use agent1_core::{
    now, EventType, ExecutionStep, OrchestrationId, OrchestrationSession, OrchestrationStatus,
    Result, RuntimeEvent, StepId, StepStatus,
};
use agent1_db::SqliteStore;
use futures_util::future::join_all;
use serde_json::json;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

#[derive(Debug, Clone)]
struct StepExecutionResult {
    step_id: StepId,
    output: Option<String>,
    error: Option<String>,
    escalated: bool,
    escalated_description: Option<String>,
}

pub struct Orchestrator {
    store: SqliteStore,
    decomposer: GoalDecomposer,
    team_manager: TeamManager,
    progress: ProgressTracker,
    escalation: EscalationManager,
    config: OrchestratorConfig,
}

impl Orchestrator {
    pub fn new(store: SqliteStore, config: OrchestratorConfig) -> Self {
        Self {
            store: store.clone(),
            decomposer: GoalDecomposer::new(config.clone()),
            team_manager: TeamManager::new(store.clone(), config.clone()),
            progress: ProgressTracker::new(store.clone()),
            escalation: EscalationManager::new(store, config.clone()),
            config,
        }
    }

    pub async fn orchestrate(&self, request: OrchestrateRequest) -> Result<OrchestrateResponse> {
        let workspace_root = request
            .workspace_root
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut session = OrchestrationSession::new(request.objective.clone());

        self.progress.save_orchestration(&session).await?;

        self.emit(
            &session.id,
            None,
            EventType::OrchestrationStarted,
            json!({"objective": request.objective}),
        )
        .await?;

        session.status = OrchestrationStatus::Planning;
        self.progress.save_orchestration(&session).await?;

        let plan_view = self
            .decomposer
            .decompose(&request.objective, &session.id)
            .await?;
        self.progress
            .save_plan(&plan_view.plan, &plan_view.steps)
            .await?;

        session.plan_id = Some(plan_view.plan.id.clone());
        session.status = OrchestrationStatus::Executing;
        self.progress.save_orchestration(&session).await?;

        self.execute_plan(
            &session,
            &plan_view.steps,
            &workspace_root,
            request.auto_approve,
        )
        .await?;

        let (mut completed_plan, completed_steps) = self
            .progress
            .get_plan_with_steps(&plan_view.plan.id)
            .await?;
        let all_complete = self.progress.is_plan_complete(&completed_steps);
        completed_plan.status = if all_complete {
            agent1_core::PlanStatus::Completed
        } else {
            agent1_core::PlanStatus::Failed
        };
        completed_plan.completed_at = Some(now());
        self.progress
            .save_plan(&completed_plan, &completed_steps)
            .await?;

        session.status = if all_complete {
            OrchestrationStatus::Completed
        } else {
            OrchestrationStatus::Failed
        };

        if session.completed_at.is_none() {
            session.completed_at = Some(now());
        }
        self.progress.save_orchestration(&session).await?;

        Ok(OrchestrateResponse {
            orchestration_id: session.id,
            plan_id: plan_view.plan.id,
            status: format!("{:?}", session.status),
            message: if all_complete {
                "Orchestration completed successfully".to_string()
            } else {
                "Orchestration completed with some steps failing".to_string()
            },
        })
    }

    async fn execute_plan(
        &self,
        session: &OrchestrationSession,
        steps: &[ExecutionStep],
        workspace_root: &PathBuf,
        auto_approve: bool,
    ) -> Result<()> {
        let mut steps = steps.to_vec();
        let mut completed_ids: HashSet<String> = HashSet::new();
        let max_concurrent = self.config.max_concurrent_agents;

        let orchestrator = Arc::new(self.clone());
        let session_id = session.id.clone();

        while !steps.is_empty() {
            let ready_steps: Vec<_> = self.decomposer.get_ready_steps(&steps, &completed_ids);

            if ready_steps.is_empty() {
                let has_pending = steps.iter().any(|s| s.status == StepStatus::Pending);
                let has_blocked = steps.iter().any(|s| s.status == StepStatus::Blocked);

                if has_pending && !has_blocked {
                    break;
                }
                if has_blocked {
                    for step in steps.iter_mut() {
                        if step.status == StepStatus::Pending
                            && step.dependencies.iter().all(|d| completed_ids.contains(d))
                        {
                        } else if step.status == StepStatus::Pending {
                            step.status = StepStatus::Blocked;
                        }
                    }
                }
                break;
            }

            let batch: Vec<_> = ready_steps.into_iter().take(max_concurrent).collect();

            let mut tasks = Vec::new();
            let mut steps_with_agents: Vec<(ExecutionStep, agent1_core::Agent)> = Vec::new();

            for step in batch {
                if let Some(role) = step.assigned_role {
                    let agent = self
                        .team_manager
                        .create_agent_for_role(role, &session.id)
                        .await?;

                    self.emit(
                        &session.id,
                        None,
                        EventType::AgentCreated,
                        json!({"agent_id": agent.id, "role": role.as_str()}),
                    )
                    .await?;

                    let mut step = step.clone();
                    step.assigned_agent_id = Some(agent.id.clone());

                    self.progress.save_step(&step).await?;

                    steps_with_agents.push((step, agent));
                }
            }

            for (mut step, _agent) in steps_with_agents {
                let orchestrator_clone = orchestrator.clone();
                let session_id_clone = session_id.clone();
                let workspace = workspace_root.clone();

                let task = tokio::spawn(async move {
                    if let Some(escalation) = check_escalation_triggers(&step.description) {
                        let esc_record = agent1_core::EscalationRecord::new(
                            session_id_clone.clone(),
                            Some(step.id.clone()),
                            escalation.0,
                            escalation.1.clone(),
                            json!({"step": step.description}),
                        );

                        if let Err(e) = orchestrator_clone
                            .escalation
                            .save_escalation(&esc_record)
                            .await
                        {
                            tracing::error!("failed to save escalation: {}", e);
                        }

                        let _ = orchestrator_clone
                            .emit(
                                &session_id_clone,
                                None,
                                EventType::EscalationCreated,
                                json!({"escalation_id": esc_record.id, "step_id": step.id}),
                            )
                            .await;

                        return StepExecutionResult {
                            step_id: step.id,
                            output: None,
                            error: None,
                            escalated: true,
                            escalated_description: Some(escalation.1),
                        };
                    }

                    step.start();

                    if let Err(e) = orchestrator_clone.progress.save_step(&step).await {
                        tracing::error!("failed to save step start: {}", e);
                    }

                    let _ = orchestrator_clone
                        .emit(
                            &session_id_clone,
                            None,
                            EventType::StepStarted,
                            json!({"step_id": step.id, "description": step.description}),
                        )
                        .await;

                    match orchestrator_clone
                        .team_manager
                        .run_step(&step, workspace, auto_approve)
                        .await
                    {
                        Ok(output) => {
                            step.complete(output);
                            StepExecutionResult {
                                step_id: step.id,
                                output: Some(step.output.clone().unwrap_or_default()),
                                error: None,
                                escalated: false,
                                escalated_description: None,
                            }
                        }
                        Err(err) => {
                            step.fail(err.to_string());
                            StepExecutionResult {
                                step_id: step.id,
                                output: None,
                                error: Some(err.to_string()),
                                escalated: false,
                                escalated_description: None,
                            }
                        }
                    }
                });

                tasks.push(task);
            }

            let results = join_all(tasks).await;

            for result in results {
                match result {
                    Ok(exec_result) => {
                        let step = steps
                            .iter_mut()
                            .find(|s| s.id == exec_result.step_id)
                            .expect("step should exist in steps list");

                        if exec_result.escalated {
                            step.status = StepStatus::Blocked;
                            if let Err(e) = self.progress.save_step(step).await {
                                tracing::error!("failed to save escalated step: {}", e);
                            }
                            let _ = self.emit(
                                &session.id,
                                None,
                                EventType::StepCompleted,
                                json!({"step_id": step.id, "status": "blocked", "escalation": exec_result.escalated_description}),
                            ).await;
                        } else if let Some(error) = exec_result.error {
                            step.status = StepStatus::Failed;
                            step.output = Some(error.clone());
                            step.completed_at = Some(now());
                            if let Err(e) = self.progress.save_step(step).await {
                                tracing::error!("failed to save failed step: {}", e);
                            }
                            let _ = self
                                .emit(
                                    &session.id,
                                    None,
                                    EventType::StepCompleted,
                                    json!({"step_id": step.id, "status": "failed", "error": error}),
                                )
                                .await;
                            completed_ids.insert(exec_result.step_id);
                        } else {
                            step.status = StepStatus::Completed;
                            step.output = exec_result.output;
                            step.completed_at = Some(now());
                            if let Err(e) = self.progress.save_step(step).await {
                                tracing::error!("failed to save completed step: {}", e);
                            }
                            let _ = self
                                .emit(
                                    &session.id,
                                    None,
                                    EventType::StepCompleted,
                                    json!({"step_id": step.id, "status": "completed"}),
                                )
                                .await;
                            completed_ids.insert(exec_result.step_id);
                        }
                    }
                    Err(join_error) => {
                        tracing::error!("task join error: {}", join_error);
                    }
                }
            }

            steps.retain(|s| !completed_ids.contains(&s.id));
        }

        Ok(())
    }

    async fn emit(
        &self,
        session_id: &str,
        agent_id: Option<&str>,
        event_type: EventType,
        payload: serde_json::Value,
    ) -> Result<()> {
        self.store
            .save_event(&RuntimeEvent {
                id: agent1_core::new_id("evt"),
                session_id: Some(session_id.to_string()),
                agent_id: agent_id.map(String::from),
                event_type,
                payload,
                created_at: now(),
            })
            .await
    }

    pub async fn get_status(
        &self,
        orchestration_id: &OrchestrationId,
    ) -> Result<OrchestrationSession> {
        self.progress.get_orchestration(orchestration_id).await
    }

    pub async fn list_active(&self) -> Result<Vec<OrchestrationSession>> {
        self.progress.list_orchestrations(50).await
    }

    pub async fn cancel(&self, orchestration_id: &OrchestrationId) -> Result<()> {
        let mut session = self.progress.get_orchestration(orchestration_id).await?;
        session.status = OrchestrationStatus::Cancelled;
        session.completed_at = Some(now());
        self.progress.save_orchestration(&session).await?;

        self.team_manager.terminate_all().await?;

        Ok(())
    }
}

impl Clone for Orchestrator {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            decomposer: GoalDecomposer::new(self.config.clone()),
            team_manager: TeamManager::new(self.store.clone(), self.config.clone()),
            progress: ProgressTracker::new(self.store.clone()),
            escalation: EscalationManager::new(self.store.clone(), self.config.clone()),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn simple_orchestration_flow() {
        let temp = tempfile::TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let store = SqliteStore::connect(&db_path).await.unwrap();

        let orchestrator = Orchestrator::new(store, OrchestratorConfig::default());

        let result = orchestrator
            .orchestrate(OrchestrateRequest {
                objective: "Say hello and finish".to_string(),
                workspace_root: Some(".".to_string()),
                auto_approve: true,
            })
            .await;

        assert!(
            result.is_ok(),
            "orchestration should succeed: {:?}",
            result.err()
        );
    }
}
