pub mod escalation;
pub mod goal_decomposer;
pub mod orchestrator;
pub mod progress_tracker;
pub mod team_manager;
pub mod types;

pub use escalation::EscalationManager;
pub use goal_decomposer::GoalDecomposer;
pub use orchestrator::Orchestrator;
pub use progress_tracker::ProgressTracker;
pub use team_manager::TeamManager;
pub use types::{
    OrchestrateRequest, OrchestrateResponse, OrchestratorConfig, PlanView, StepUpdate,
};

use agent1_core::Result;
use agent1_db::SqliteStore;
use std::path::Path;

pub async fn create_orchestrator(db_path: impl AsRef<Path>) -> Result<Orchestrator> {
    let db_path = db_path.as_ref();
    let store = SqliteStore::connect(db_path).await?;
    let memory_path = db_path.with_extension("memory.db");
    let orchestrator = Orchestrator::new(store, OrchestratorConfig::default());
    Ok(orchestrator.with_memory(&memory_path).await)
}

pub async fn run_orchestration(
    db_path: impl AsRef<Path>,
    objective: &str,
    workspace_root: Option<String>,
    auto_approve: bool,
) -> Result<OrchestrateResponse> {
    let db_path = db_path.as_ref();
    let store = SqliteStore::connect(db_path).await?;

    let memory_path = db_path.with_extension("memory.db");
    let orchestrator = Orchestrator::new(store, OrchestratorConfig::default());
    let orchestrator = orchestrator.with_memory(&memory_path).await;

    orchestrator
        .orchestrate(OrchestrateRequest {
            objective: objective.to_string(),
            workspace_root,
            auto_approve,
        })
        .await
}
