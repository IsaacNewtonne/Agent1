//! Agent1 External Gateway
//!
//! Handles external agent connections for the collaboration workspace:
//! - Invite generation with project-scoped access
//! - Inbound WebSocket connections from friend agents
//! - Permission enforcement on every action
//! - Heartbeat tracking and presence monitoring
//! - JSON-RPC style protocol over WebSocket

use std::collections::HashMap;
use std::sync::Arc;

use agent1_collab::CollaborationEngine;
use agent1_core::{now, Agent1Error, AuthorType, ExternalPermissions, InviteToken, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

/// An active connection from an external agent
#[derive(Debug, Clone)]
pub struct ExternalConnection {
    pub external_agent_id: String,
    pub project_id: String,
    pub agent_name: String,
    pub permissions: ExternalPermissions,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    pub active_tasks: u32,
}

/// The external agent gateway
pub struct ExternalGateway {
    engine: Arc<CollaborationEngine>,
    connections: Arc<RwLock<HashMap<String, ExternalConnection>>>,
}

impl ExternalGateway {
    pub fn new(engine: Arc<CollaborationEngine>) -> Self {
        Self {
            engine,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate an invite token for a project
    pub async fn generate_invite(
        &self,
        project_id: &str,
        permissions: ExternalPermissions,
        created_by: String,
    ) -> Result<InviteToken> {
        self.engine
            .generate_invite(project_id, permissions, created_by)
            .await
    }

    /// Validate a token and register the connection
    pub async fn authenticate(
        &self,
        token: &str,
        agent_name: String,
    ) -> Result<ExternalConnection> {
        let external = self.engine.accept_invite(token, agent_name.clone()).await?;
        let now_ts = now();

        let connection = ExternalConnection {
            external_agent_id: external.id.clone(),
            project_id: external.project_id.clone(),
            agent_name,
            permissions: external.permissions.clone(),
            connected_at: now_ts,
            last_heartbeat: now_ts,
            active_tasks: 0,
        };

        // Register in connection map
        {
            let mut conns = self.connections.write().await;
            conns.insert(external.id.clone(), connection.clone());
        }

        // Register presence
        self.engine
            .register_presence(&external.id, &external.project_id, AuthorType::External)
            .await;

        Ok(connection)
    }

    /// Handle an incoming message from an external agent
    pub async fn handle_message(
        &self,
        external_id: &str,
        message: GatewayMessage,
    ) -> Result<GatewayResponse> {
        let connection = {
            let conns = self.connections.read().await;
            conns
                .get(external_id)
                .cloned()
                .ok_or_else(|| Agent1Error::PermissionDenied("not connected".to_string()))?
        };

        match message {
            GatewayMessage::Heartbeat => {
                self.engine.update_external_heartbeat(external_id).await?;
                {
                    let mut conns = self.connections.write().await;
                    if let Some(conn) = conns.get_mut(external_id) {
                        conn.last_heartbeat = now();
                    }
                }
                Ok(GatewayResponse::Ack)
            }

            GatewayMessage::ReadBlackboard { key } => {
                if !connection.permissions.can_read_blackboard {
                    return Err(Agent1Error::PermissionDenied(
                        "cannot read blackboard".to_string(),
                    ));
                }
                if let Some(key) = key {
                    let entry = self
                        .engine
                        .blackboard_get(&connection.project_id, &key)
                        .await;
                    Ok(GatewayResponse::BlackboardEntry { entry })
                } else {
                    let entries = self.engine.blackboard_read(&connection.project_id).await;
                    Ok(GatewayResponse::BlackboardEntries { entries })
                }
            }

            GatewayMessage::WriteBlackboard { key, value } => {
                if !connection.permissions.can_write_blackboard {
                    return Err(Agent1Error::PermissionDenied(
                        "cannot write blackboard".to_string(),
                    ));
                }
                let entry = self
                    .engine
                    .blackboard_write(
                        &connection.project_id,
                        key,
                        value,
                        external_id.to_string(),
                        AuthorType::External,
                    )
                    .await?;
                Ok(GatewayResponse::Written { id: entry.id })
            }

            GatewayMessage::SubmitContribution {
                description,
                output,
            } => {
                // Submit as a completed task contribution
                let mut task = self
                    .engine
                    .submit_task(&connection.project_id, description)
                    .await?;
                task = self
                    .engine
                    .assign_task(&task.id, external_id, AuthorType::External)
                    .await?;
                if let Some(output) = output {
                    task = self.engine.complete_task(&task.id, output).await?;
                }
                Ok(GatewayResponse::TaskCreated { task_id: task.id })
            }

            GatewayMessage::Disconnect => {
                self.disconnect(external_id).await?;
                Ok(GatewayResponse::Ack)
            }
        }
    }

    /// Disconnect an external agent
    pub async fn disconnect(&self, external_id: &str) -> Result<()> {
        let connection = {
            let mut conns = self.connections.write().await;
            conns.remove(external_id)
        };

        if let Some(conn) = connection {
            self.engine
                .disconnect_external(&conn.project_id, external_id)
                .await?;
        }

        Ok(())
    }

    /// Get all active connections for a project
    pub async fn project_connections(&self, project_id: &str) -> Vec<ExternalConnection> {
        let conns = self.connections.read().await;
        conns
            .values()
            .filter(|c| c.project_id == project_id)
            .cloned()
            .collect()
    }

    /// Check for stale connections (no heartbeat in 30s)
    pub async fn cleanup_stale_connections(&self) {
        let stale_threshold = chrono::Duration::seconds(30);
        let now_ts = now();

        let stale_ids: Vec<String> = {
            let conns = self.connections.read().await;
            conns
                .iter()
                .filter(|(_, c)| now_ts - c.last_heartbeat > stale_threshold)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in stale_ids {
            let _ = self.disconnect(&id).await;
            tracing::info!(external_id = %id, "cleaned up stale external connection");
        }
    }
}

/// Messages from external agents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayMessage {
    Heartbeat,
    ReadBlackboard {
        key: Option<String>,
    },
    WriteBlackboard {
        key: String,
        value: Value,
    },
    SubmitContribution {
        description: String,
        output: Option<String>,
    },
    Disconnect,
}

/// Responses to external agents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayResponse {
    Ack,
    BlackboardEntry {
        entry: Option<agent1_core::BlackboardEntry>,
    },
    BlackboardEntries {
        entries: Vec<agent1_core::BlackboardEntry>,
    },
    Written {
        id: String,
    },
    TaskCreated {
        task_id: String,
    },
    Error {
        message: String,
    },
}
