use crate::types::{check_escalation_triggers, OrchestratorConfig};
use agent1_core::{
    Agent1Error, EscalationId, EscalationRecord, EscalationStatus, EscalationType, OrchestrationId,
    Result, StepId,
};
use agent1_db::SqliteStore;
use serde_json::Value;
use sqlx::Row;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct EscalationManager {
    store: SqliteStore,
    pending_escalations: Mutex<HashMap<EscalationId, EscalationRecord>>,
}

impl EscalationManager {
    pub fn new(store: SqliteStore, _config: OrchestratorConfig) -> Self {
        Self {
            store,
            pending_escalations: Mutex::new(HashMap::new()),
        }
    }

    pub async fn check_and_create_escalation(
        &self,
        orchestration_id: OrchestrationId,
        step_id: Option<StepId>,
        content: &str,
        payload: Value,
    ) -> Result<Option<EscalationRecord>> {
        if let Some((escalation_type, description)) = check_escalation_triggers(content) {
            let escalation = EscalationRecord::new(
                orchestration_id,
                step_id,
                escalation_type,
                description,
                payload,
            );

            self.save_escalation(&escalation).await?;

            let mut pending = self.pending_escalations.lock().await;
            pending.insert(escalation.id.clone(), escalation.clone());

            Ok(Some(escalation))
        } else {
            Ok(None)
        }
    }

    pub async fn save_escalation(&self, escalation: &EscalationRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO escalation_queue (
                id, orchestration_id, step_id, escalation_type, description,
                payload, status, response, created_at, resolved_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                response = excluded.response,
                resolved_at = excluded.resolved_at
            "#,
        )
        .bind(&escalation.id)
        .bind(&escalation.orchestration_id)
        .bind(&escalation.step_id)
        .bind(serde_json::to_string(&escalation.escalation_type).unwrap_or_default())
        .bind(&escalation.description)
        .bind(escalation.payload.to_string())
        .bind(serde_json::to_string(&escalation.status).unwrap_or_default())
        .bind(&escalation.response)
        .bind(escalation.created_at)
        .bind(escalation.resolved_at)
        .execute(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save escalation: {err}")))?;

        Ok(())
    }

    pub async fn get_escalation(&self, id: &EscalationId) -> Result<EscalationRecord> {
        let pending = self.pending_escalations.lock().await;
        if let Some(escalation) = pending.get(id) {
            return Ok(escalation.clone());
        }

        let row = sqlx::query(
            r#"
            SELECT id, orchestration_id, step_id, escalation_type, description,
                   payload, status, response, created_at, resolved_at
            FROM escalation_queue
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_one(self.store.pool())
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to get escalation: {err}")))?;

        escalation_from_row(row)
    }

    pub async fn resolve_escalation(&self, id: &EscalationId, response: &str) -> Result<()> {
        let mut escalation = self.get_escalation(id).await?;
        escalation.resolve(response.to_string());
        self.save_escalation(&escalation).await?;

        let mut pending = self.pending_escalations.lock().await;
        pending.remove(id);

        Ok(())
    }

    pub async fn decline_escalation(&self, id: &EscalationId, reason: &str) -> Result<()> {
        let mut escalation = self.get_escalation(id).await?;
        escalation.decline(reason.to_string());
        self.save_escalation(&escalation).await?;

        let mut pending = self.pending_escalations.lock().await;
        pending.remove(id);

        Ok(())
    }

    pub async fn list_pending_escalations(
        &self,
        orchestration_id: Option<&OrchestrationId>,
    ) -> Result<Vec<EscalationRecord>> {
        let pending = self.pending_escalations.lock().await;
        let mut result: Vec<EscalationRecord> = pending
            .values()
            .filter(|e| {
                if let Some(oid) = orchestration_id {
                    e.orchestration_id == *oid
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(result)
    }

    pub async fn has_pending_escalations(
        &self,
        orchestration_id: &OrchestrationId,
    ) -> Result<bool> {
        let pending = self
            .list_pending_escalations(Some(orchestration_id))
            .await?;
        Ok(!pending.is_empty())
    }

    pub fn should_escalate_content(&self, content: &str) -> bool {
        check_escalation_triggers(content).is_some()
    }

    pub fn escalation_type_priority(&self, escalation_type: EscalationType) -> u8 {
        match escalation_type {
            EscalationType::Security => 10,
            EscalationType::Finance => 9,
            EscalationType::Identity => 8,
            EscalationType::Access => 7,
            EscalationType::External => 6,
            EscalationType::Approval => 5,
        }
    }
}

fn escalation_from_row(row: sqlx::sqlite::SqliteRow) -> Result<EscalationRecord> {
    let escalation_type_text: String = row.get("escalation_type");
    let escalation_type: EscalationType =
        serde_json::from_str(&escalation_type_text).unwrap_or(EscalationType::Approval);

    let status_text: String = row.get("status");
    let status: EscalationStatus =
        serde_json::from_str(&status_text).unwrap_or(EscalationStatus::Pending);

    let payload_json: String = row.get("payload");
    let payload = serde_json::from_str(&payload_json).unwrap_or(serde_json::Value::Null);

    Ok(EscalationRecord {
        id: row.get("id"),
        orchestration_id: row.get("orchestration_id"),
        step_id: row.get("step_id"),
        escalation_type,
        description: row.get("description"),
        payload,
        status,
        response: row.get("response"),
        created_at: row.get("created_at"),
        resolved_at: row.get("resolved_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalation_priority() {
        let temp = tempfile::TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = runtime.block_on(SqliteStore::connect(&db_path)).unwrap();
        let manager = EscalationManager::new(store, OrchestratorConfig::default());

        assert!(manager.should_escalate_content("send me the api_key please"));
        assert!(manager.should_escalate_content("process payment for $100"));
        assert!(manager.should_escalate_content("connect to my gmail account"));
        assert!(!manager.should_escalate_content("just read the file"));
    }
}
