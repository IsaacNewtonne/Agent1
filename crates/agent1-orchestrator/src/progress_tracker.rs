use agent1_core::{
    ExecutionStep, OrchestrationSession, OrchestrationStatus, PlanStatus,
    StepStatus, now, Agent1Error, Result,
};
use agent1_db::SqliteStore;
use sqlx::Row;

pub struct ProgressTracker {
    store: SqliteStore,
}

impl ProgressTracker {
    pub fn new(store: SqliteStore) -> Self {
        Self { store }
    }

    pub async fn save_orchestration(&self, session: &OrchestrationSession) -> Result<()> {
        let _now = now();
        sqlx::query(
            r#"
            INSERT INTO orchestration_sessions (
                id, objective, plan_id, status, created_at, updated_at, completed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                objective = excluded.objective,
                plan_id = excluded.plan_id,
                status = excluded.status,
                updated_at = excluded.updated_at,
                completed_at = excluded.completed_at
            "#,
        )
        .bind(&session.id)
        .bind(&session.objective)
        .bind(&session.plan_id)
        .bind(serde_json::to_string(&session.status).unwrap_or_default())
        .bind(session.created_at)
        .bind(session.updated_at)
        .bind(session.completed_at)
        .execute(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save orchestration: {err}")))?;
        Ok(())
    }

    pub async fn save_plan(&self, plan: &agent1_core::ExecutionPlan, steps: &[ExecutionStep]) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO execution_plans (
                id, orchestration_id, objective, raw_goal, status, created_at, completed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                completed_at = excluded.completed_at
            "#,
        )
        .bind(&plan.id)
        .bind(&plan.orchestration_id)
        .bind(&plan.objective)
        .bind(&plan.raw_goal)
        .bind(serde_json::to_string(&plan.status).unwrap_or_default())
        .bind(plan.created_at)
        .bind(plan.completed_at)
        .execute(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save plan: {err}")))?;

        for step in steps {
            self.save_step(step).await?;
        }

        Ok(())
    }

    pub async fn save_step(&self, step: &ExecutionStep) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO execution_steps (
                id, plan_id, description, step_order, assigned_agent_id,
                assigned_role, dependencies, status, output, review_notes,
                created_at, started_at, completed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                assigned_agent_id = excluded.assigned_agent_id,
                assigned_role = excluded.assigned_role,
                status = excluded.status,
                output = excluded.output,
                review_notes = excluded.review_notes,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at
            "#,
        )
        .bind(&step.id)
        .bind(&step.plan_id)
        .bind(&step.description)
        .bind(step.step_order as i64)
        .bind(&step.assigned_agent_id)
        .bind(step.assigned_role.as_ref().map(|r| r.as_str()))
        .bind(serde_json::to_string(&step.dependencies).unwrap_or_default())
        .bind(serde_json::to_string(&step.status).unwrap_or_default())
        .bind(&step.output)
        .bind(&step.review_notes)
        .bind(step.created_at)
        .bind(step.started_at)
        .bind(step.completed_at)
        .execute(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save step: {err}")))?;
        Ok(())
    }

    pub async fn get_orchestration(&self, id: &str) -> Result<OrchestrationSession> {
        let row = sqlx::query(
            r#"
            SELECT id, objective, plan_id, status, created_at, updated_at, completed_at
            FROM orchestration_sessions
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_one(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to get orchestration: {err}")))?;

        orchestration_from_row(row)
    }

    pub async fn list_orchestrations(&self, limit: i64) -> Result<Vec<OrchestrationSession>> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, plan_id, status, created_at, updated_at, completed_at
            FROM orchestration_sessions
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list orchestrations: {err}")))?;

        rows.into_iter().map(orchestration_from_row).collect()
    }

    pub async fn get_plan_with_steps(&self, plan_id: &str) -> Result<(agent1_core::ExecutionPlan, Vec<ExecutionStep>)> {
        let plan_row = sqlx::query(
            r#"
            SELECT id, orchestration_id, objective, raw_goal, status, created_at, completed_at
            FROM execution_plans
            WHERE id = ?1
            "#,
        )
        .bind(plan_id)
        .fetch_one(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to get plan: {err}")))?;

        let plan = plan_from_row(plan_row)?;

        let step_rows = sqlx::query(
            r#"
            SELECT id, plan_id, description, step_order, assigned_agent_id,
                   assigned_role, dependencies, status, output, review_notes,
                   created_at, started_at, completed_at
            FROM execution_steps
            WHERE plan_id = ?1
            ORDER BY step_order ASC
            "#,
        )
        .bind(plan_id)
        .fetch_all(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to get steps: {err}")))?;

        let steps = step_rows.into_iter().map(step_from_row).collect::<Result<Vec<_>>>()?;

        Ok((plan, steps))
    }

    pub fn calculate_progress<'a>(&self, steps: &'a [ExecutionStep]) -> (usize, usize, &'a [ExecutionStep]) {
        let total = steps.len();
        let completed = steps.iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count();
        let _blocked = steps.iter()
            .filter(|s| s.status == StepStatus::Blocked)
            .count();
        (completed, total, steps)
    }

    pub fn is_plan_complete(&self, steps: &[ExecutionStep]) -> bool {
        steps.iter().all(|s| s.status == StepStatus::Completed)
    }

    pub fn get_blocked_steps<'a>(&self, steps: &'a [ExecutionStep]) -> Vec<&'a ExecutionStep> {
        steps.iter().filter(|s| s.status == StepStatus::Blocked).collect()
    }
}

fn orchestration_from_row(row: sqlx::sqlite::SqliteRow) -> Result<OrchestrationSession> {
    let status_text: String = row.get("status");
    let status: OrchestrationStatus = serde_json::from_str(&status_text)
        .unwrap_or(OrchestrationStatus::Received);

    Ok(OrchestrationSession {
        id: row.get("id"),
        objective: row.get("objective"),
        plan_id: row.get("plan_id"),
        status,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    })
}

fn plan_from_row(row: sqlx::sqlite::SqliteRow) -> Result<agent1_core::ExecutionPlan> {
    let status_text: String = row.get("status");
    let status: PlanStatus = serde_json::from_str(&status_text)
        .unwrap_or(PlanStatus::Draft);

    Ok(agent1_core::ExecutionPlan {
        id: row.get("id"),
        orchestration_id: row.get("orchestration_id"),
        objective: row.get("objective"),
        raw_goal: row.get("raw_goal"),
        status,
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    })
}

fn step_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ExecutionStep> {
    let status_text: String = row.get("status");
    let status: StepStatus = serde_json::from_str(&status_text)
        .unwrap_or(StepStatus::Pending);

    let assigned_role_text: Option<String> = row.get("assigned_role");
    let assigned_role = assigned_role_text.and_then(|r| {
        match r.as_str() {
            "orchestrator" => Some(agent1_core::AgentRole::Orchestrator),
            "planner" => Some(agent1_core::AgentRole::Planner),
            "worker" => Some(agent1_core::AgentRole::Worker),
            "critic" => Some(agent1_core::AgentRole::Critic),
            "researcher" => Some(agent1_core::AgentRole::Researcher),
            "builder" => Some(agent1_core::AgentRole::Builder),
            "reporter" => Some(agent1_core::AgentRole::Reporter),
            _ => None,
        }
    });

    let dependencies_json: String = row.get("dependencies");
    let dependencies: Vec<String> = serde_json::from_str(&dependencies_json).unwrap_or_default();

    Ok(ExecutionStep {
        id: row.get("id"),
        plan_id: row.get("plan_id"),
        description: row.get("description"),
        step_order: row.get::<i64, _>("step_order") as usize,
        assigned_agent_id: row.get("assigned_agent_id"),
        assigned_role,
        dependencies,
        status,
        output: row.get("output"),
        review_notes: row.get("review_notes"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn calculate_progress() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let store = SqliteStore::connect(&db_path).await.unwrap();
        let tracker = ProgressTracker::new(store);

        let steps = vec![
            ExecutionStep::new("plan1".to_string(), "Step 1".to_string(), 0, vec![]),
            ExecutionStep::new("plan1".to_string(), "Step 2".to_string(), 1, vec![]),
            ExecutionStep::new("plan1".to_string(), "Step 3".to_string(), 2, vec![]),
        ];

        let (completed, total, _) = tracker.calculate_progress(&steps);
        assert_eq!(completed, 0);
        assert_eq!(total, 3);

        let is_complete = tracker.is_plan_complete(&steps);
        assert!(!is_complete);
    }
}