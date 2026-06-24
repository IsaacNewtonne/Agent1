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
use agent1_memory::{
    create_memory_entry, MemoryProvider, MemorySearchQuery, MemoryType, SemanticMemoryStore,
};
use futures_util::future::join_all;
use serde_json::json;
use std::{collections::HashSet, path::Path, path::PathBuf, sync::Arc};

#[derive(Debug, Clone)]
struct StepExecutionResult {
    step_id: StepId,
    output: Option<String>,
    error: Option<String>,
    escalated: bool,
    escalated_description: Option<String>,
    critique_result: Option<CritiqueResult>,
}

#[derive(Debug, Clone)]
enum CritiqueResult {
    Approved,
    NeedsRevision { reason: String },
    Failed { reason: String },
}

pub struct Orchestrator {
    store: SqliteStore,
    memory: Option<SemanticMemoryStore>,
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
            memory: None,
            decomposer: GoalDecomposer::new(config.clone()),
            team_manager: TeamManager::new(store.clone(), config.clone()),
            progress: ProgressTracker::new(store.clone()),
            escalation: EscalationManager::new(store, config.clone()),
            config,
        }
    }

    pub async fn with_memory(mut self, memory_path: impl AsRef<Path>) -> Self {
        match SemanticMemoryStore::connect(memory_path).await {
            Ok(store) => {
                self.memory = Some(store);
                tracing::info!("Memory store connected successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to connect memory store: {}", e);
            }
        }
        self
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

        // Recall relevant memories before planning
        let relevant_context = self.recall_relevant_context(&request.objective).await;
        if !relevant_context.is_empty() {
            tracing::info!(
                "Recalled {} relevant memories for context",
                relevant_context.len()
            );
        }

        session.status = OrchestrationStatus::Planning;
        self.progress.save_orchestration(&session).await?;

        let plan_view = self
            .decomposer
            .decompose_with_context(&request.objective, &session.id, &relevant_context)
            .await?;
        self.progress
            .save_plan(&plan_view.plan, &plan_view.steps)
            .await?;

        for (sub_plan_id, sub_steps) in &plan_view.sub_steps {
            self.progress
                .save_sub_steps(sub_plan_id, sub_steps.to_vec())
                .await?;
        }

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

        // Store outcome in memory
        self.store_outcome(&request.objective, &completed_plan, &completed_steps)
            .await;

        // Generate suggestions based on the execution
        self.generate_suggestions(&completed_plan, &completed_steps, &session.id)
            .await;

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

    async fn recall_relevant_context(&self, objective: &str) -> Vec<String> {
        let Some(ref memory) = self.memory else {
            return Vec::new();
        };

        match memory
            .search(MemorySearchQuery {
                query: objective.to_string(),
                memory_type: None,
                limit: 5,
                min_relevance: 0.6,
            })
            .await
        {
            Ok(results) => results.into_iter().map(|r| r.entry.content).collect(),
            Err(e) => {
                tracing::warn!("Failed to recall memories: {}", e);
                Vec::new()
            }
        }
    }

    async fn store_outcome(
        &self,
        objective: &str,
        plan: &agent1_core::ExecutionPlan,
        steps: &[ExecutionStep],
    ) {
        let Some(ref memory) = self.memory else {
            return;
        };

        let successful_steps: Vec<_> = steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .map(|s| s.description.clone())
            .collect();

        if successful_steps.is_empty() {
            return;
        }

        let outcome = format!(
            "Objective: {}\nCompleted steps: {}",
            objective,
            successful_steps.join("; ")
        );

        let embedding = match self.get_embedding(&outcome).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to embed outcome: {}", e);
                return;
            }
        };

        let entry = create_memory_entry(
            outcome,
            embedding,
            MemoryType::Task,
            if plan.status == agent1_core::PlanStatus::Completed {
                0.7
            } else {
                0.4
            },
        );

        if let Err(e) = memory.store(entry).await {
            tracing::warn!("Failed to store outcome in memory: {}", e);
        }
    }

    async fn generate_suggestions(
        &self,
        plan: &agent1_core::ExecutionPlan,
        steps: &[ExecutionStep],
        session_id: &str,
    ) {
        let Some(ref memory) = self.memory else {
            return;
        };

        let failed_steps: Vec<_> = steps
            .iter()
            .filter(|s| s.status == StepStatus::Failed)
            .collect();

        for step in failed_steps {
            if step.output.is_none() {
                continue;
            }

            let suggestion = agent1_core::Suggestion::new(
                agent1_core::SuggestionType::FollowUp,
                format!("Retry or alternative approach for: {}", step.description),
                format!(
                    "Failed step in plan {} - original error: {}",
                    plan.id,
                    step.output.as_ref().unwrap_or(&String::new())
                ),
                None,
            );

            if let Err(e) = memory.store_suggestion(&suggestion).await {
                tracing::warn!("Failed to store suggestion: {}", e);
            } else {
                tracing::debug!(
                    "Generated follow-up suggestion for failed step: {}",
                    step.id
                );
                let _ = self.emit(
                    session_id,
                    None,
                    EventType::SuggestionCreated,
                    json!({"suggestion": suggestion}),
                );
            }
        }

        if plan.status != agent1_core::PlanStatus::Completed && !steps.is_empty() {
            let pending_count = steps
                .iter()
                .filter(|s| s.status == StepStatus::Pending || s.status == StepStatus::Blocked)
                .count();
            if pending_count > 0 {
                let suggestion = agent1_core::Suggestion::new(
                    agent1_core::SuggestionType::FollowUp,
                    format!(
                        "Continue incomplete objective: {} ({} steps remaining)",
                        plan.objective, pending_count
                    ),
                    format!(
                        "Plan {} was not completed - {} of {} steps remain",
                        plan.id,
                        pending_count,
                        steps.len()
                    ),
                    None,
                );

                if let Err(e) = memory.store_suggestion(&suggestion).await {
                    tracing::warn!("Failed to store suggestion: {}", e);
                } else {
                    tracing::debug!(
                        "Generated follow-up suggestion for incomplete plan: {}",
                        plan.id
                    );
                    let _ = self.emit(
                        session_id,
                        None,
                        EventType::SuggestionCreated,
                        json!({"suggestion": suggestion}),
                    );
                }
            }
        }

        self.generate_improvement_suggestions(memory, plan, steps, session_id)
            .await;
        self.generate_routine_suggestions(memory, plan, steps, session_id)
            .await;
        self.generate_contextual_suggestions(memory, plan, steps, session_id)
            .await;
    }

    async fn generate_improvement_suggestions(
        &self,
        memory: &agent1_memory::SemanticMemoryStore,
        plan: &agent1_core::ExecutionPlan,
        steps: &[ExecutionStep],
        session_id: &str,
    ) {
        let completed_steps: Vec<_> = steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .collect();

        if completed_steps.len() <= 1 {
            return;
        }

        let step_descriptions: String = completed_steps
            .iter()
            .take(5)
            .map(|s| s.description.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        if completed_steps.len() >= 3 {
            let suggestion = agent1_core::Suggestion::new(
                agent1_core::SuggestionType::Improvement,
                format!("Optimize workflow for: {}", plan.objective.chars().take(50).collect::<String>()),
                format!("This plan executed {} steps: {}. Consider caching intermediate results or parallelizing independent steps for faster execution.", completed_steps.len(), step_descriptions),
                None,
            );

            if let Err(e) = memory.store_suggestion(&suggestion).await {
                tracing::warn!("Failed to store improvement suggestion: {}", e);
            } else {
                tracing::debug!("Generated improvement suggestion for plan: {}", plan.id);
                let _ = self.emit(
                    session_id,
                    None,
                    EventType::SuggestionCreated,
                    json!({"suggestion": suggestion}),
                );
            }
        }

        let long_steps: Vec<_> = completed_steps
            .iter()
            .filter(|s| {
                if let (Some(start), Some(end)) = (s.started_at, s.completed_at) {
                    let duration = end.signed_duration_since(start);
                    duration.num_seconds() > 120
                } else {
                    false
                }
            })
            .collect();

        if long_steps.len() >= 2 {
            let slow_descriptions: String = long_steps
                .iter()
                .map(|s| s.description.as_str())
                .take(3)
                .collect::<Vec<_>>()
                .join(", ");

            let suggestion = agent1_core::Suggestion::new(
                agent1_core::SuggestionType::Improvement,
                format!("Speed up slow steps: {}", slow_descriptions.chars().take(40).collect::<String>()),
                format!("{} steps took over 2 minutes each. Consider using faster models, caching context, or breaking these steps into smaller sub-tasks.", long_steps.len()),
                None,
            );

            if let Err(e) = memory.store_suggestion(&suggestion).await {
                tracing::warn!("Failed to store speed improvement suggestion: {}", e);
            } else {
                let _ = self.emit(
                    session_id,
                    None,
                    EventType::SuggestionCreated,
                    json!({"suggestion": suggestion}),
                );
            }
        }
    }

    async fn generate_routine_suggestions(
        &self,
        memory: &agent1_memory::SemanticMemoryStore,
        _plan: &agent1_core::ExecutionPlan,
        _steps: &[ExecutionStep],
        session_id: &str,
    ) {
        let task_memories = match memory.list(Some(agent1_memory::MemoryType::Task), 20).await {
            Ok(mems) => mems,
            Err(e) => {
                tracing::warn!(
                    "Failed to list task memories for routine suggestions: {}",
                    e
                );
                return;
            }
        };

        if task_memories.len() < 3 {
            return;
        }

        let recent_task_count = task_memories
            .iter()
            .filter(|m| {
                let age = chrono::Utc::now() - m.created_at;
                age.num_days() < 7
            })
            .count();

        if recent_task_count >= 3 {
            let task_summary: String = task_memories
                .iter()
                .take(5)
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join(" | ");

            let suggestion = agent1_core::Suggestion::new(
                agent1_core::SuggestionType::Routine,
                "Establish routine for recurring tasks".to_string(),
                format!("Detected {} similar tasks in the past week suggesting a pattern. Consider creating a routine agent or template for: {}", recent_task_count, task_summary.chars().take(100).collect::<String>()),
                None,
            );

            if let Err(e) = memory.store_suggestion(&suggestion).await {
                tracing::warn!("Failed to store routine suggestion: {}", e);
            } else {
                tracing::debug!(
                    "Generated routine suggestion based on {} recent tasks",
                    recent_task_count
                );
                let _ = self.emit(
                    session_id,
                    None,
                    EventType::SuggestionCreated,
                    json!({"suggestion": suggestion}),
                );
            }
        }

        let pattern_keywords = ["build", "test", "deploy", "review", "analyze"];
        let mut keyword_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();

        for mem in &task_memories {
            let content_lower = mem.content.to_lowercase();
            for keyword in &pattern_keywords {
                if content_lower.contains(keyword) {
                    *keyword_counts.entry(keyword).or_insert(0) += 1;
                }
            }
        }

        if let Some((most_common, _)) = keyword_counts.iter().max_by_key(|(_, count)| *count) {
            if *keyword_counts.get(most_common).unwrap_or(&0) >= 3 {
                let suggestion = agent1_core::Suggestion::new(
                    agent1_core::SuggestionType::Routine,
                    format!("Create {} workflow template", most_common),
                    format!("'{}' appears in {} recent tasks. Creating a reusable workflow template could save time.", most_common, keyword_counts.get(most_common).unwrap()),
                    None,
                );

                if let Err(e) = memory.store_suggestion(&suggestion).await {
                    tracing::warn!("Failed to store {} routine suggestion: {}", most_common, e);
                } else {
                    let _ = self.emit(
                        session_id,
                        None,
                        EventType::SuggestionCreated,
                        json!({"suggestion": suggestion}),
                    );
                }
            }
        }
    }

    async fn generate_contextual_suggestions(
        &self,
        memory: &agent1_memory::SemanticMemoryStore,
        plan: &agent1_core::ExecutionPlan,
        _steps: &[ExecutionStep],
        session_id: &str,
    ) {
        let query = agent1_memory::MemorySearchQuery {
            query: plan.objective.clone(),
            memory_type: None,
            limit: 3,
            min_relevance: 0.5,
        };

        let related_results = match memory.search(query).await {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("Failed to search for related memories: {}", e);
                return;
            }
        };

        if related_results.is_empty() {
            return;
        }

        let related_summary: String = related_results
            .iter()
            .take(3)
            .map(|r| r.entry.content.as_str())
            .collect::<Vec<_>>()
            .join(" | ");

        let similarity_avg: f32 = related_results.iter().map(|r| r.similarity).sum::<f32>()
            / related_results.len() as f32;

        if similarity_avg > 0.7 {
            let suggestion = agent1_core::Suggestion::new(
                agent1_core::SuggestionType::Contextual,
                format!(
                    "Building on past work: {}",
                    plan.objective.chars().take(40).collect::<String>()
                ),
                format!(
                    "Found {} related memories with high similarity ({:.1}%): {}",
                    related_results.len(),
                    similarity_avg * 100.0,
                    related_summary.chars().take(150).collect::<String>()
                ),
                Some(
                    related_results
                        .first()
                        .map(|r| r.entry.id.clone())
                        .unwrap_or_default(),
                ),
            );

            if let Err(e) = memory.store_suggestion(&suggestion).await {
                tracing::warn!("Failed to store contextual suggestion: {}", e);
            } else {
                tracing::debug!(
                    "Generated contextual suggestion with {} related memories",
                    related_results.len()
                );
                let _ = self.emit(
                    session_id,
                    None,
                    EventType::SuggestionCreated,
                    json!({"suggestion": suggestion}),
                );
            }
        }
    }

    async fn get_suggestions(
        &self,
        status: Option<agent1_core::SuggestionStatus>,
    ) -> Result<Vec<agent1_core::Suggestion>> {
        let Some(ref memory) = self.memory else {
            return Ok(Vec::new());
        };
        memory.get_suggestions(status, 20).await.map_err(|e| {
            agent1_core::Agent1Error::Runtime(format!("Failed to get suggestions: {}", e))
        })
    }

    async fn update_suggestion_status(
        &self,
        id: &str,
        status: agent1_core::SuggestionStatus,
    ) -> Result<()> {
        let Some(ref memory) = self.memory else {
            return Ok(());
        };
        memory
            .update_suggestion_status(id, status)
            .await
            .map_err(|e| {
                agent1_core::Agent1Error::Runtime(format!("Failed to update suggestion: {}", e))
            })
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let config = &self.config.model_routing.planner;
        let provider = agent1_models::provider_for(config)?;
        provider.embeddings(text, config).await.map_err(|e| {
            agent1_core::Agent1Error::Runtime(format!("Failed to get embedding: {}", e))
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
                            critique_result: None,
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
                                critique_result: None,
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
                                critique_result: None,
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
                            step.output = exec_result.output.clone();
                            step.completed_at = Some(now());

                            let critique = self.critique_step(step).await;

                            let critique_result_json = match &critique {
                                CritiqueResult::Approved => json!({"critique": "approved"}),
                                CritiqueResult::NeedsRevision { reason } => {
                                    json!({"critique": "needs_revision", "reason": reason})
                                }
                                CritiqueResult::Failed { reason } => {
                                    json!({"critique": "failed", "reason": reason})
                                }
                            };

                            if let Err(e) = self.progress.save_step(step).await {
                                tracing::error!("failed to save completed step: {}", e);
                            }
                            let _ = self
                                .emit(
                                    &session.id,
                                    None,
                                    EventType::StepCompleted,
                                    json!({"step_id": step.id, "status": "completed", "critique": critique_result_json}),
                                )
                                .await;

                            match critique {
                                CritiqueResult::NeedsRevision { reason } => {
                                    tracing::info!("Step {} needs revision: {}", step.id, reason);
                                }
                                CritiqueResult::Failed { reason } => {
                                    tracing::warn!("Step {} failed critique: {}", step.id, reason);
                                    step.output = Some(format!(
                                        "{} [CRITIQUE FAILED: {}]",
                                        step.output.clone().unwrap_or_default(),
                                        reason
                                    ));
                                    step.status = StepStatus::Failed;
                                    let _ = self.progress.save_step(step).await;
                                }
                                CritiqueResult::Approved => {}
                            }

                            if let Some(sub_plan_id) = &step.sub_plan_id {
                                match self
                                    .execute_sub_plan(step, session, workspace_root, auto_approve)
                                    .await
                                {
                                    Ok(sub_output) => {
                                        tracing::info!(
                                            "Sub-plan {} completed for step {}",
                                            sub_plan_id,
                                            step.id
                                        );
                                        step.output = Some(format!(
                                            "{}\n\n[SUB-PLAN {} COMPLETED]\n{}",
                                            step.output.clone().unwrap_or_default(),
                                            sub_plan_id,
                                            sub_output
                                        ));
                                        let _ = self.progress.save_step(step).await;
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Sub-plan {} failed for step {}: {}",
                                            sub_plan_id,
                                            step.id,
                                            e
                                        );
                                    }
                                }
                            }

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

    async fn critique_step(&self, step: &ExecutionStep) -> CritiqueResult {
        let output = match &step.output {
            Some(o) => o,
            None => {
                return CritiqueResult::Failed {
                    reason: "No output produced".to_string(),
                }
            }
        };

        let critique_prompt = format!(
            r#"You are a critic agent reviewing the output of an executed step.

Step Description: {}

Step Output:
{}

Evaluate the output and respond with ONLY a JSON object:
{{
  "approved": true/false,
  "reason": "brief explanation of your evaluation",
  "needs_revision": false (include only if approved is false)
}}

Rules:
1. If the output directly addresses the step description and is actionable, approve it
2. If the output is incomplete, unclear, or doesn't fully address the step, mark as needs_revision
3. If the output is completely wrong, contradictory, or nonsensical, mark as failed
4. Keep reason concise (1-2 sentences)
5. Return ONLY valid JSON, no markdown or explanation."#,
            step.description, output
        );

        let config = &self.config.model_routing.critic;
        let provider = match agent1_models::provider_for(config) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to get critic provider: {}", e);
                return CritiqueResult::Approved;
            }
        };

        let request = agent1_core::ChatRequest {
            model: config.clone(),
            messages: vec![agent1_core::ChatMessage {
                role: "user".to_string(),
                content: critique_prompt,
            }],
        };

        let response = match provider.chat(request).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Critic call failed: {}", e);
                return CritiqueResult::Approved;
            }
        };

        match self.parse_critique(&response.content) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("Failed to parse critique: {}", e);
                CritiqueResult::Approved
            }
        }
    }

    fn parse_critique(&self, content: &str) -> Result<CritiqueResult> {
        let trimmed = content.trim();
        let cleaned = if trimmed.starts_with("```json") {
            trimmed
                .trim_start_matches("```json")
                .trim_end_matches("```")
                .trim()
        } else {
            trimmed
        };

        let json: serde_json::Value = serde_json::from_str(cleaned).map_err(|e| {
            agent1_core::Agent1Error::InvalidModelResponse(format!("invalid JSON: {}", e))
        })?;

        let approved = json
            .get("approved")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let reason = json
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("No reason provided")
            .to_string();

        if approved {
            Ok(CritiqueResult::Approved)
        } else if json
            .get("needs_revision")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            Ok(CritiqueResult::NeedsRevision { reason })
        } else {
            Ok(CritiqueResult::Failed { reason })
        }
    }

    async fn execute_sub_plan(
        &self,
        step: &ExecutionStep,
        session: &OrchestrationSession,
        workspace: &Path,
        auto_approve: bool,
    ) -> Result<String> {
        let Some(ref sub_plan_id) = step.sub_plan_id else {
            return Ok(step.output.clone().unwrap_or_default());
        };

        tracing::info!("Executing sub-plan {} for step {}", sub_plan_id, step.id);

        let mut sub_outputs = Vec::new();

        if let Some(sub_steps_data) = self.progress.get_sub_steps(sub_plan_id).await? {
            for sub_step in sub_steps_data {
                let role = sub_step
                    .assigned_role
                    .unwrap_or(agent1_core::AgentRole::Worker);

                let agent = self
                    .team_manager
                    .create_agent_for_role(role, &session.id)
                    .await?;

                let mut step_with_agent = sub_step.clone();
                step_with_agent.assigned_agent_id = Some(agent.id.clone());
                self.progress.save_step(&step_with_agent).await?;

                match self
                    .team_manager
                    .run_step(&step_with_agent, workspace.to_path_buf(), auto_approve)
                    .await
                {
                    Ok(output) => {
                        sub_outputs.push(format!("[{}] {}", sub_step.description, output));
                        tracing::debug!("Sub-step {} completed", sub_step.id);
                    }
                    Err(e) => {
                        sub_outputs.push(format!("[{}] FAILED: {}", sub_step.description, e));
                        tracing::warn!("Sub-step {} failed: {}", sub_step.id, e);
                    }
                }
            }
        } else {
            tracing::warn!("No sub-steps found for sub-plan {}", sub_plan_id);
        }

        Ok(sub_outputs.join("\n"))
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
            memory: None,
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
