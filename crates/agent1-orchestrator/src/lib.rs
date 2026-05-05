pub mod types;
pub mod goal_decomposer;
pub mod team_manager;
pub mod progress_tracker;
pub mod escalation;
pub mod orchestrator;

pub use goal_decomposer::GoalDecomposer;
pub use team_manager::TeamManager;
pub use progress_tracker::ProgressTracker;
pub use escalation::EscalationManager;
pub use orchestrator::Orchestrator;
pub use types::{OrchestrateRequest, OrchestrateResponse, OrchestratorConfig, PlanView, StepUpdate};

use agent1_core::Result;
use agent1_db::SqliteStore;
use std::path::Path;

pub async fn create_orchestrator(db_path: impl AsRef<Path>) -> Result<Orchestrator> {
    let store = SqliteStore::connect(db_path).await?;
    Ok(Orchestrator::new(store, OrchestratorConfig::default()))
}

pub async fn run_orchestration(
    db_path: impl AsRef<Path>,
    objective: &str,
    workspace_root: Option<String>,
    auto_approve: bool,
) -> Result<OrchestrateResponse> {
    let store = SqliteStore::connect(db_path).await?;
    let orchestrator = Orchestrator::new(store, OrchestratorConfig::default());

    orchestrator.orchestrate(OrchestrateRequest {
        objective: objective.to_string(),
        workspace_root,
        auto_approve,
    }).await
}

mod agent1_core {
    pub use agent1_core::*;
}

mod agent1_db {
    pub use agent1_db::*;
}