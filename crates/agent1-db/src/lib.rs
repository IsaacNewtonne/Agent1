use std::{path::Path, str::FromStr};

use agent1_core::{
    now, redact_secrets_text, redact_secrets_value, Agent, Agent1Error, AgentCard, ApprovalRecord,
    BlackboardEntry, CollabEvent, CollabTask, CollaborationMode, EventType, ExternalAgent,
    ExternalAgentStatus, InviteToken, McpServerConfig, MemoryItem, Message, MessageRole, Project,
    Result, RuntimeEvent, Session, SessionStatus, ToolCallRecord, ToolCallStatus,
};
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteConnectOptions, Executor, Row, SqlitePool};

const INITIAL_SCHEMA: &str = include_str!("../migrations/0001_initial.sql");
const ORCHESTRATOR_SCHEMA: &str = include_str!("../migrations/0002_orchestrator.sql");
const COLLABORATION_SCHEMA: &str = include_str!("../migrations/0003_collaboration.sql");

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                Agent1Error::Runtime(format!("failed to create data directory: {err}"))
            })?;
        }
        let options = SqliteConnectOptions::from_str(&path.to_string_lossy())
            .map_err(|err| Agent1Error::Runtime(format!("invalid sqlite path: {err}")))?
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to connect sqlite: {err}")))?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<()> {
        self.ensure_pre_initial_compat().await?;
        self.pool.execute(INITIAL_SCHEMA).await.map_err(|err| {
            Agent1Error::Runtime(format!("failed to run initial migration: {err}"))
        })?;
        self.pool
            .execute(ORCHESTRATOR_SCHEMA)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to run orchestrator migration: {err}"))
            })?;
        self.pool
            .execute(COLLABORATION_SCHEMA)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to run collaboration migration: {err}"))
            })?;
        self.ensure_compat_columns().await?;
        Ok(())
    }

    async fn ensure_pre_initial_compat(&self) -> Result<()> {
        let has_sessions = sqlx::query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions' LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| {
            Agent1Error::Runtime(format!("failed to inspect existing sessions table: {err}"))
        })?
        .is_some();

        if has_sessions {
            self.ensure_sessions_project_id().await?;
        }
        Ok(())
    }

    async fn ensure_compat_columns(&self) -> Result<()> {
        self.ensure_sessions_project_id().await?;
        self.pool
            .execute("CREATE INDEX IF NOT EXISTS idx_sessions_project_id ON sessions(project_id)")
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to index sessions.project_id: {err}"))
            })?;
        Ok(())
    }

    async fn ensure_sessions_project_id(&self) -> Result<()> {
        let columns = sqlx::query("PRAGMA table_info(sessions)")
            .fetch_all(&self.pool)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to inspect sessions schema: {err}"))
            })?;
        let has_project_id = columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "project_id");
        if !has_project_id {
            self.pool
                .execute("ALTER TABLE sessions ADD COLUMN project_id TEXT")
                .await
                .map_err(|err| {
                    Agent1Error::Runtime(format!("failed to add sessions.project_id: {err}"))
                })?;
        }
        Ok(())
    }

    pub async fn save_agent(&self, agent: &Agent) -> Result<()> {
        let now = now();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, description, role, system_prompt, model_config_json,
                tools_json, memory_config_json, permissions_json, max_iterations,
                created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                role = excluded.role,
                system_prompt = excluded.system_prompt,
                model_config_json = excluded.model_config_json,
                tools_json = excluded.tools_json,
                memory_config_json = excluded.memory_config_json,
                permissions_json = excluded.permissions_json,
                max_iterations = excluded.max_iterations,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&agent.id)
        .bind(&agent.name)
        .bind(&agent.description)
        .bind(&agent.role)
        .bind(&agent.system_prompt)
        .bind(json_string(&agent.model)?)
        .bind(json_string(&agent.tools)?)
        .bind(json_string(&agent.memory)?)
        .bind(json_string(&agent.permissions)?)
        .bind(agent.max_iterations as i64)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save agent: {err}")))?;
        Ok(())
    }

    pub async fn get_agent(&self, agent_id: &str) -> Result<Agent> {
        let row = sqlx::query(
            r#"
            SELECT id, name, description, role, system_prompt, model_config_json,
                   tools_json, memory_config_json, permissions_json, max_iterations
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(agent_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Agent1Error::AgentNotFound(format!("{agent_id}: {err}")))?;
        agent_from_row(row)
    }

    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, role, system_prompt, model_config_json,
                   tools_json, memory_config_json, permissions_json, max_iterations
            FROM agents
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list agents: {err}")))?;
        rows.into_iter().map(agent_from_row).collect()
    }

    pub async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(|err| {
            Agent1Error::Runtime(format!("failed to start delete agent transaction: {err}"))
        })?;

        sqlx::query("DELETE FROM agent_cards WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to delete agent card: {err}")))?;

        let result = sqlx::query("DELETE FROM agents WHERE id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to delete agent: {err}")))?;

        if result.rows_affected() == 0 {
            return Err(Agent1Error::AgentNotFound(agent_id.to_string()));
        }

        tx.commit().await.map_err(|err| {
            Agent1Error::Runtime(format!("failed to commit delete agent transaction: {err}"))
        })?;
        Ok(())
    }

    pub async fn create_session(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, title, project_id, root_agent_id, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&session.id)
        .bind(&session.title)
        .bind(&session.project_id)
        .bind(&session.root_agent_id)
        .bind(json_name(&session.status)?)
        .bind(session.created_at)
        .bind(session.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to create session: {err}")))?;
        Ok(())
    }

    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: SessionStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE sessions SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(json_name(&status)?)
            .bind(now())
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to update session status: {err}"))
            })?;
        Ok(())
    }

    pub async fn create_session_shell(
        &self,
        root_agent_id: &str,
        title: Option<String>,
        project_id: Option<String>,
    ) -> Result<Session> {
        let created_at = now();
        let session = Session {
            id: agent1_core::new_id("sess"),
            title,
            project_id,
            root_agent_id: root_agent_id.to_string(),
            status: SessionStatus::Running,
            created_at,
            updated_at: created_at,
        };
        self.create_session(&session).await?;
        Ok(session)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Session> {
        let row = sqlx::query(
            r#"
            SELECT id, title, project_id, root_agent_id, status, created_at, updated_at
            FROM sessions
            WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| {
            Agent1Error::Runtime(format!("failed to read session `{session_id}`: {err}"))
        })?;
        session_from_row(row)
    }

    pub async fn recent_sessions(&self, limit: i64) -> Result<Vec<Session>> {
        let rows = sqlx::query(
            r#"
            SELECT id, title, project_id, root_agent_id, status, created_at, updated_at
            FROM sessions
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read sessions: {err}")))?;

        rows.into_iter().map(session_from_row).collect()
    }

    pub async fn save_message(&self, message: &Message) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO messages (
                id, session_id, from_agent_id, to_agent_id, role, content, metadata_json, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&message.id)
        .bind(&message.session_id)
        .bind(&message.from_agent_id)
        .bind(&message.to_agent_id)
        .bind(json_name(&message.role)?)
        .bind(redact_secrets_text(&message.content))
        .bind(redact_secrets_value(&message.metadata).to_string())
        .bind(message.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save message: {err}")))?;
        Ok(())
    }

    pub async fn save_event(&self, event: &RuntimeEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO events (id, session_id, agent_id, event_type, payload_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&event.id)
        .bind(&event.session_id)
        .bind(&event.agent_id)
        .bind(json_name(&event.event_type)?)
        .bind(redact_secrets_value(&event.payload).to_string())
        .bind(event.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save event: {err}")))?;
        Ok(())
    }

    pub async fn save_tool_call(&self, call: &ToolCallRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tool_calls (
                id, session_id, agent_id, tool_name, input_json, output_json,
                status, error, started_at, finished_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&call.id)
        .bind(&call.session_id)
        .bind(&call.agent_id)
        .bind(&call.tool_name)
        .bind(redact_secrets_value(&call.input).to_string())
        .bind(
            call.output
                .as_ref()
                .map(|value| redact_secrets_value(value).to_string()),
        )
        .bind(json_name(&call.status)?)
        .bind(&call.error)
        .bind(call.started_at)
        .bind(call.finished_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save tool call: {err}")))?;
        Ok(())
    }

    pub async fn recent_events(&self, limit: i64) -> Result<Vec<RuntimeEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, agent_id, event_type, payload_json, created_at
            FROM events
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read events: {err}")))?;

        rows.into_iter()
            .map(|row| {
                let event_type_text: String = row.get("event_type");
                let event_type: EventType = serde_json::from_value(serde_json::Value::String(
                    event_type_text,
                ))
                .map_err(|err| Agent1Error::Runtime(format!("invalid event type in db: {err}")))?;
                let payload_json: String = row.get("payload_json");
                let payload = serde_json::from_str(&payload_json).map_err(|err| {
                    Agent1Error::Runtime(format!("invalid event payload in db: {err}"))
                })?;
                Ok(RuntimeEvent {
                    id: row.get("id"),
                    session_id: row.get("session_id"),
                    agent_id: row.get("agent_id"),
                    event_type,
                    payload,
                    created_at: row.get::<DateTime<Utc>, _>("created_at"),
                })
            })
            .collect()
    }

    pub async fn session_events(&self, session_id: &str) -> Result<Vec<RuntimeEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, agent_id, event_type, payload_json, created_at
            FROM events
            WHERE session_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read session events: {err}")))?;

        rows.into_iter().map(event_from_row).collect()
    }

    pub async fn session_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, from_agent_id, to_agent_id, role, content, metadata_json, created_at
            FROM messages
            WHERE session_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read session messages: {err}")))?;

        rows.into_iter()
            .map(|row| {
                let role_text: String = row.get("role");
                let role: MessageRole = parse_json_string_enum(&role_text, "message role")?;
                let metadata_json: Option<String> = row.get("metadata_json");
                let metadata = metadata_json
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|err| {
                        Agent1Error::Runtime(format!("invalid message metadata: {err}"))
                    })?
                    .unwrap_or_default();
                Ok(Message {
                    id: row.get("id"),
                    session_id: row.get("session_id"),
                    from_agent_id: row.get("from_agent_id"),
                    to_agent_id: row.get("to_agent_id"),
                    role,
                    content: row.get("content"),
                    metadata,
                    created_at: row.get::<DateTime<Utc>, _>("created_at"),
                })
            })
            .collect()
    }

    pub async fn session_tool_calls(&self, session_id: &str) -> Result<Vec<ToolCallRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, agent_id, tool_name, input_json, output_json,
                   status, error, started_at, finished_at
            FROM tool_calls
            WHERE session_id = ?1
            ORDER BY started_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read tool calls: {err}")))?;

        rows.into_iter()
            .map(|row| {
                let status_text: String = row.get("status");
                let status: ToolCallStatus =
                    parse_json_string_enum(&status_text, "tool call status")?;
                let input_json: String = row.get("input_json");
                let output_json: Option<String> = row.get("output_json");
                Ok(ToolCallRecord {
                    id: row.get("id"),
                    session_id: row.get("session_id"),
                    agent_id: row.get("agent_id"),
                    tool_name: row.get("tool_name"),
                    input: serde_json::from_str(&input_json).map_err(|err| {
                        Agent1Error::Runtime(format!("invalid tool input JSON: {err}"))
                    })?,
                    output: output_json
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()
                        .map_err(|err| {
                            Agent1Error::Runtime(format!("invalid tool output JSON: {err}"))
                        })?,
                    status,
                    error: row.get("error"),
                    started_at: row.get::<DateTime<Utc>, _>("started_at"),
                    finished_at: row.get("finished_at"),
                })
            })
            .collect()
    }

    pub async fn write_memory(&self, item: &MemoryItem) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO memories (
                id, scope, agent_id, content, tags_json, embedding_json,
                importance, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                scope = excluded.scope,
                agent_id = excluded.agent_id,
                content = excluded.content,
                tags_json = excluded.tags_json,
                embedding_json = excluded.embedding_json,
                importance = excluded.importance,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&item.id)
        .bind(&item.scope)
        .bind(&item.agent_id)
        .bind(redact_secrets_text(&item.content))
        .bind(json_string(&item.tags)?)
        .bind(item.embedding.as_ref().map(|value| value.to_string()))
        .bind(item.importance)
        .bind(item.created_at)
        .bind(item.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to write memory: {err}")))?;
        Ok(())
    }

    pub async fn search_memories(
        &self,
        agent_id: Option<&str>,
        query: &str,
        limit: i64,
    ) -> Result<Vec<MemoryItem>> {
        let pattern = format!("%{}%", query.trim());
        let rows = if query.trim().is_empty() {
            sqlx::query(
                r#"
                SELECT id, scope, agent_id, content, tags_json, embedding_json,
                       importance, created_at, updated_at
                FROM memories
                WHERE (?1 IS NULL OR agent_id = ?1 OR scope = 'global')
                ORDER BY importance DESC, updated_at DESC
                LIMIT ?2
                "#,
            )
            .bind(agent_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT id, scope, agent_id, content, tags_json, embedding_json,
                       importance, created_at, updated_at
                FROM memories
                WHERE (?1 IS NULL OR agent_id = ?1 OR scope = 'global')
                  AND (content LIKE ?2 OR tags_json LIKE ?2)
                ORDER BY importance DESC, updated_at DESC
                LIMIT ?3
                "#,
            )
            .bind(agent_id)
            .bind(pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|err| Agent1Error::Runtime(format!("failed to search memories: {err}")))?;
        rows.into_iter().map(memory_from_row).collect()
    }

    pub async fn delete_memory(&self, memory_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM memories WHERE id = ?1")
            .bind(memory_id)
            .execute(&self.pool)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to delete memory: {err}")))?;
        Ok(())
    }

    pub async fn save_mcp_server(&self, server: &McpServerConfig) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO mcp_servers (
                id, name, transport, command, args_json, env_json, enabled, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                transport = excluded.transport,
                command = excluded.command,
                args_json = excluded.args_json,
                env_json = excluded.env_json,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&server.id)
        .bind(&server.name)
        .bind(&server.transport)
        .bind(&server.command)
        .bind(json_string(&server.args)?)
        .bind(json_string(&server.env)?)
        .bind(server.enabled)
        .bind(server.created_at)
        .bind(server.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save MCP server: {err}")))?;
        Ok(())
    }

    pub async fn list_mcp_servers(&self) -> Result<Vec<McpServerConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, transport, command, args_json, env_json,
                   enabled, created_at, updated_at
            FROM mcp_servers
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list MCP servers: {err}")))?;
        rows.into_iter().map(mcp_server_from_row).collect()
    }

    pub async fn get_mcp_server(&self, id: &str) -> Result<McpServerConfig> {
        let row = sqlx::query(
            r#"
            SELECT id, name, transport, command, args_json, env_json,
                   enabled, created_at, updated_at
            FROM mcp_servers
            WHERE id = ?1 OR name = ?1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read MCP server `{id}`: {err}")))?;
        mcp_server_from_row(row)
    }

    pub async fn delete_mcp_server(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM mcp_servers WHERE id = ?1 OR name = ?1")
            .bind(id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to delete MCP server `{id}`: {err}"))
            })?;
        Ok(())
    }

    pub async fn update_mcp_server_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        sqlx::query(
            "UPDATE mcp_servers SET enabled = ?1, updated_at = ?2 WHERE id = ?3 OR name = ?3",
        )
        .bind(enabled)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|err| {
            Agent1Error::Runtime(format!("failed to update MCP server `{id}`: {err}"))
        })?;
        Ok(())
    }

    pub async fn save_agent_card(&self, card: &AgentCard) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO agent_cards (agent_id, card_json, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(agent_id) DO UPDATE SET
                card_json = excluded.card_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&card.id)
        .bind(json_string(card)?)
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save agent card: {err}")))?;
        Ok(())
    }

    pub async fn list_agent_cards(&self) -> Result<Vec<AgentCard>> {
        let rows = sqlx::query("SELECT card_json FROM agent_cards ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to list agent cards: {err}")))?;
        rows.into_iter()
            .map(|row| {
                let card_json: String = row.get("card_json");
                serde_json::from_str(&card_json)
                    .map_err(|err| Agent1Error::Runtime(format!("invalid agent card JSON: {err}")))
            })
            .collect()
    }

    pub async fn find_agent_cards_by_skill(&self, skill: &str) -> Result<Vec<AgentCard>> {
        let skill = skill.to_ascii_lowercase();
        let cards = self.list_agent_cards().await?;
        Ok(cards
            .into_iter()
            .filter(|card| {
                card.skills.iter().any(|item| {
                    item.name.to_ascii_lowercase().contains(&skill)
                        || item.description.to_ascii_lowercase().contains(&skill)
                })
            })
            .collect())
    }

    pub async fn save_approval_request(&self, approval: &ApprovalRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO approvals (
                id, session_id, agent_id, request_json, decision, decided_at, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                request_json = excluded.request_json,
                decision = excluded.decision,
                decided_at = excluded.decided_at
            "#,
        )
        .bind(&approval.id)
        .bind(&approval.session_id)
        .bind(&approval.agent_id)
        .bind(redact_secrets_value(&approval.request).to_string())
        .bind(&approval.decision)
        .bind(approval.decided_at)
        .bind(approval.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save approval: {err}")))?;
        Ok(())
    }

    pub async fn update_approval_decision(&self, approval_id: &str, decision: &str) -> Result<()> {
        sqlx::query("UPDATE approvals SET decision = ?1, decided_at = ?2 WHERE id = ?3")
            .bind(decision)
            .bind(now())
            .bind(approval_id)
            .execute(&self.pool)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to update approval: {err}")))?;
        Ok(())
    }

    pub async fn get_approval(&self, approval_id: &str) -> Result<ApprovalRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, session_id, agent_id, request_json, decision, decided_at, created_at
            FROM approvals
            WHERE id = ?1
            "#,
        )
        .bind(approval_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| {
            Agent1Error::Runtime(format!("failed to read approval `{approval_id}`: {err}"))
        })?;
        approval_from_row(row)
    }

    pub async fn recent_approvals(&self, limit: i64) -> Result<Vec<ApprovalRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, session_id, agent_id, request_json, decision, decided_at, created_at
            FROM approvals
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list approvals: {err}")))?;
        rows.into_iter().map(approval_from_row).collect()
    }

    // ─── Collaboration: Projects ───

    pub async fn save_project(&self, project: &Project) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO projects (id, name, description, collab_mode, agent_ids_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                collab_mode = excluded.collab_mode,
                agent_ids_json = excluded.agent_ids_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.description)
        .bind(json_name(&project.collaboration_mode)?)
        .bind(json_string(&project.local_agent_ids)?)
        .bind(project.created_at)
        .bind(project.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save project: {err}")))?;
        Ok(())
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Project> {
        let row = sqlx::query(
            "SELECT id, name, description, collab_mode, agent_ids_json, created_at, updated_at FROM projects WHERE id = ?1",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("project not found `{project_id}`: {err}")))?;
        project_from_row(row)
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, collab_mode, agent_ids_json, created_at, updated_at FROM projects ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list projects: {err}")))?;
        rows.into_iter().map(project_from_row).collect()
    }

    pub async fn delete_project(&self, project_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM projects WHERE id = ?1")
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to delete project: {err}")))?;
        Ok(())
    }

    // ─── Collaboration: Blackboard ───

    pub async fn save_blackboard_entry(&self, entry: &BlackboardEntry) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO blackboard (id, project_id, key, value_json, author_agent_id, author_type, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(project_id, key) DO UPDATE SET
                value_json = excluded.value_json,
                author_agent_id = excluded.author_agent_id,
                author_type = excluded.author_type,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&entry.id)
        .bind(&entry.project_id)
        .bind(&entry.key)
        .bind(entry.value.to_string())
        .bind(&entry.author_agent_id)
        .bind(json_name(&entry.author_type)?)
        .bind(entry.created_at)
        .bind(entry.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save blackboard entry: {err}")))?;
        Ok(())
    }

    pub async fn get_blackboard(&self, project_id: &str) -> Result<Vec<BlackboardEntry>> {
        let rows = sqlx::query(
            "SELECT id, project_id, key, value_json, author_agent_id, author_type, created_at, updated_at FROM blackboard WHERE project_id = ?1 ORDER BY key",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to read blackboard: {err}")))?;
        rows.into_iter().map(blackboard_from_row).collect()
    }

    // ─── Collaboration: External Agents ───

    pub async fn save_external_agent(&self, agent: &ExternalAgent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO external_agents (id, project_id, name, endpoint, invite_token, capabilities_json, permissions_json, status, last_heartbeat, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                endpoint = excluded.endpoint,
                capabilities_json = excluded.capabilities_json,
                permissions_json = excluded.permissions_json,
                status = excluded.status,
                last_heartbeat = excluded.last_heartbeat
            "#,
        )
        .bind(&agent.id)
        .bind(&agent.project_id)
        .bind(&agent.name)
        .bind(&agent.endpoint)
        .bind(&agent.invite_token)
        .bind(json_string(&agent.capabilities)?)
        .bind(json_string(&agent.permissions)?)
        .bind(json_name(&agent.status)?)
        .bind(agent.last_heartbeat)
        .bind(agent.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save external agent: {err}")))?;
        Ok(())
    }

    pub async fn list_external_agents(&self, project_id: &str) -> Result<Vec<ExternalAgent>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, endpoint, invite_token, capabilities_json, permissions_json, status, last_heartbeat, created_at FROM external_agents WHERE project_id = ?1 ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list external agents: {err}")))?;
        rows.into_iter().map(external_agent_from_row).collect()
    }

    pub async fn update_external_status(
        &self,
        external_id: &str,
        status: ExternalAgentStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE external_agents SET status = ?1 WHERE id = ?2")
            .bind(json_name(&status)?)
            .bind(external_id)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to update external status: {err}"))
            })?;
        Ok(())
    }

    pub async fn update_external_heartbeat(&self, external_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE external_agents SET last_heartbeat = ?1, status = 'connected' WHERE id = ?2",
        )
        .bind(now())
        .bind(external_id)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to update heartbeat: {err}")))?;
        Ok(())
    }

    pub async fn delete_external_agent(&self, external_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM external_agents WHERE id = ?1")
            .bind(external_id)
            .execute(&self.pool)
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to delete external agent: {err}"))
            })?;
        Ok(())
    }

    // ─── Collaboration: Invite Tokens ───

    pub async fn save_invite_token(&self, invite: &InviteToken) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO invite_tokens (token, project_id, project_name, permissions_json, created_by, gateway_url, expires_at, used_by, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&invite.token)
        .bind(&invite.project_id)
        .bind(&invite.project_name)
        .bind(json_string(&invite.permissions)?)
        .bind(&invite.created_by)
        .bind(&invite.gateway_url)
        .bind(invite.expires_at)
        .bind(&invite.used_by)
        .bind(invite.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save invite token: {err}")))?;
        Ok(())
    }

    pub async fn get_invite_token(&self, token: &str) -> Result<InviteToken> {
        let row = sqlx::query(
            "SELECT token, project_id, project_name, permissions_json, created_by, gateway_url, expires_at, used_by, created_at FROM invite_tokens WHERE token = ?1",
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("invite token not found: {err}")))?;
        invite_from_row(row)
    }

    pub async fn mark_invite_used(&self, token: &str, used_by: &str) -> Result<()> {
        sqlx::query("UPDATE invite_tokens SET used_by = ?1 WHERE token = ?2")
            .bind(used_by)
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to mark invite used: {err}")))?;
        Ok(())
    }

    // ─── Collaboration: Tasks ───

    pub async fn list_invite_tokens(&self, project_id: &str) -> Result<Vec<InviteToken>> {
        let rows = sqlx::query(
            "SELECT token, project_id, project_name, permissions_json, created_by, gateway_url, expires_at, used_by, created_at FROM invite_tokens WHERE project_id = ?1 ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list invite tokens: {err}")))?;
        rows.into_iter().map(invite_from_row).collect()
    }

    pub async fn revoke_invite_token(&self, project_id: &str, token: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM invite_tokens WHERE project_id = ?1 AND token = ?2")
            .bind(project_id)
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to revoke invite token: {err}")))?;
        if result.rows_affected() == 0 {
            return Err(Agent1Error::Runtime("invite token not found".to_string()));
        }
        Ok(())
    }

    pub async fn save_collab_task(&self, task: &CollabTask) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO collab_tasks (id, project_id, description, assigned_agent_id, assigned_agent_type, status, output, requires_approval, created_at, completed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                assigned_agent_id = excluded.assigned_agent_id,
                assigned_agent_type = excluded.assigned_agent_type,
                status = excluded.status,
                output = excluded.output,
                completed_at = excluded.completed_at
            "#,
        )
        .bind(&task.id)
        .bind(&task.project_id)
        .bind(&task.description)
        .bind(&task.assigned_agent_id)
        .bind(task.assigned_agent_type.as_ref().map(|t| json_name(t)).transpose()?)
        .bind(json_name(&task.status)?)
        .bind(&task.output)
        .bind(task.requires_approval)
        .bind(task.created_at)
        .bind(task.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save collab task: {err}")))?;
        Ok(())
    }

    pub async fn get_collab_task(&self, task_id: &str) -> Result<CollabTask> {
        let row = sqlx::query(
            "SELECT id, project_id, description, assigned_agent_id, assigned_agent_type, status, output, requires_approval, created_at, completed_at FROM collab_tasks WHERE id = ?1",
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("collab task not found: {err}")))?;
        collab_task_from_row(row)
    }

    pub async fn list_collab_tasks(&self, project_id: &str) -> Result<Vec<CollabTask>> {
        let rows = sqlx::query(
            "SELECT id, project_id, description, assigned_agent_id, assigned_agent_type, status, output, requires_approval, created_at, completed_at FROM collab_tasks WHERE project_id = ?1 ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list collab tasks: {err}")))?;
        rows.into_iter().map(collab_task_from_row).collect()
    }

    // ─── Collaboration: Events ───

    pub async fn save_collab_event(&self, event: &CollabEvent) -> Result<()> {
        sqlx::query(
            "INSERT INTO collab_events (id, project_id, event_type, agent_id, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&event.id)
        .bind(&event.project_id)
        .bind(json_name(&event.event_type)?)
        .bind(&event.agent_id)
        .bind(event.payload.to_string())
        .bind(event.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to save collab event: {err}")))?;
        Ok(())
    }

    pub async fn recent_collab_events(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<CollabEvent>> {
        let rows = sqlx::query(
            "SELECT id, project_id, event_type, agent_id, payload_json, created_at FROM collab_events WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to list collab events: {err}")))?;
        rows.into_iter().map(collab_event_from_row).collect()
    }
}

fn agent_from_row(row: sqlx::sqlite::SqliteRow) -> Result<Agent> {
    Ok(Agent {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        role: row.get("role"),
        system_prompt: row.get("system_prompt"),
        model: serde_json::from_str(&row.get::<String, _>("model_config_json"))
            .map_err(|err| Agent1Error::Runtime(format!("invalid agent model JSON: {err}")))?,
        tools: serde_json::from_str(&row.get::<String, _>("tools_json"))
            .map_err(|err| Agent1Error::Runtime(format!("invalid agent tools JSON: {err}")))?,
        memory: serde_json::from_str(&row.get::<String, _>("memory_config_json"))
            .map_err(|err| Agent1Error::Runtime(format!("invalid agent memory JSON: {err}")))?,
        permissions: serde_json::from_str(&row.get::<String, _>("permissions_json")).map_err(
            |err| Agent1Error::Runtime(format!("invalid agent permissions JSON: {err}")),
        )?,
        max_iterations: row.get::<i64, _>("max_iterations") as u32,
    })
}

fn session_from_row(row: sqlx::sqlite::SqliteRow) -> Result<Session> {
    let status_text: String = row.get("status");
    let status: SessionStatus = parse_json_string_enum(&status_text, "session status")?;
    Ok(Session {
        id: row.get("id"),
        title: row.get("title"),
        project_id: row.get("project_id"),
        root_agent_id: row.get("root_agent_id"),
        status,
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn memory_from_row(row: sqlx::sqlite::SqliteRow) -> Result<MemoryItem> {
    let tags_json: Option<String> = row.get("tags_json");
    let embedding_json: Option<String> = row.get("embedding_json");
    Ok(MemoryItem {
        id: row.get("id"),
        scope: row.get("scope"),
        agent_id: row.get("agent_id"),
        content: row.get("content"),
        tags: tags_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|err| Agent1Error::Runtime(format!("invalid memory tags JSON: {err}")))?
            .unwrap_or_default(),
        embedding: embedding_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|err| Agent1Error::Runtime(format!("invalid memory embedding JSON: {err}")))?,
        importance: row.get("importance"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn mcp_server_from_row(row: sqlx::sqlite::SqliteRow) -> Result<McpServerConfig> {
    let args_json: Option<String> = row.get("args_json");
    let env_json: Option<String> = row.get("env_json");
    Ok(McpServerConfig {
        id: row.get("id"),
        name: row.get("name"),
        transport: row.get("transport"),
        command: row.get("command"),
        args: args_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|err| Agent1Error::Runtime(format!("invalid MCP args JSON: {err}")))?
            .unwrap_or_default(),
        env: env_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|err| Agent1Error::Runtime(format!("invalid MCP env JSON: {err}")))?
            .unwrap_or_default(),
        enabled: row.get("enabled"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn approval_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ApprovalRecord> {
    let request_json: String = row.get("request_json");
    Ok(ApprovalRecord {
        id: row.get("id"),
        session_id: row.get("session_id"),
        agent_id: row.get("agent_id"),
        request: serde_json::from_str(&request_json)
            .map_err(|err| Agent1Error::Runtime(format!("invalid approval request JSON: {err}")))?,
        decision: row.get("decision"),
        decided_at: row.get("decided_at"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    })
}

fn event_from_row(row: sqlx::sqlite::SqliteRow) -> Result<RuntimeEvent> {
    let event_type_text: String = row.get("event_type");
    let event_type: EventType = parse_json_string_enum(&event_type_text, "event type")?;
    let payload_json: String = row.get("payload_json");
    let payload = serde_json::from_str(&payload_json)
        .map_err(|err| Agent1Error::Runtime(format!("invalid event payload in db: {err}")))?;
    Ok(RuntimeEvent {
        id: row.get("id"),
        session_id: row.get("session_id"),
        agent_id: row.get("agent_id"),
        event_type,
        payload,
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    })
}

fn parse_json_string_enum<T: serde::de::DeserializeOwned>(text: &str, label: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(text.to_string()))
        .map_err(|err| Agent1Error::Runtime(format!("invalid {label} in db: {err}")))
}

fn json_string<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|err| Agent1Error::Runtime(format!("failed to serialize JSON: {err}")))
}

fn json_name<T: serde::Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value)
        .map_err(|err| Agent1Error::Runtime(format!("failed to serialize enum: {err}")))?
    {
        serde_json::Value::String(text) => Ok(text),
        other => Ok(other.to_string()),
    }
}

fn project_from_row(row: sqlx::sqlite::SqliteRow) -> Result<Project> {
    let mode_text: String = row.get("collab_mode");
    let mode: CollaborationMode = parse_json_string_enum(&mode_text, "collaboration mode")?;
    let agent_ids_json: String = row.get("agent_ids_json");
    Ok(Project {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        collaboration_mode: mode,
        local_agent_ids: serde_json::from_str(&agent_ids_json).unwrap_or_default(),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn blackboard_from_row(row: sqlx::sqlite::SqliteRow) -> Result<BlackboardEntry> {
    let value_json: String = row.get("value_json");
    let author_type_text: String = row.get("author_type");
    Ok(BlackboardEntry {
        id: row.get("id"),
        project_id: row.get("project_id"),
        key: row.get("key"),
        value: serde_json::from_str(&value_json).unwrap_or_default(),
        author_agent_id: row.get("author_agent_id"),
        author_type: parse_json_string_enum(&author_type_text, "author type")?,
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn external_agent_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ExternalAgent> {
    let caps_json: String = row.get("capabilities_json");
    let perms_json: String = row.get("permissions_json");
    let status_text: String = row.get("status");
    Ok(ExternalAgent {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        endpoint: row.get("endpoint"),
        invite_token: row.get("invite_token"),
        capabilities: serde_json::from_str(&caps_json).unwrap_or_default(),
        permissions: serde_json::from_str(&perms_json).unwrap_or_default(),
        status: parse_json_string_enum(&status_text, "external agent status")?,
        last_heartbeat: row.get("last_heartbeat"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    })
}

fn invite_from_row(row: sqlx::sqlite::SqliteRow) -> Result<InviteToken> {
    let perms_json: String = row.get("permissions_json");
    Ok(InviteToken {
        token: row.get("token"),
        project_id: row.get("project_id"),
        project_name: row.get("project_name"),
        permissions: serde_json::from_str(&perms_json).unwrap_or_default(),
        created_by: row.get("created_by"),
        gateway_url: row.get("gateway_url"),
        expires_at: row.get("expires_at"),
        used_by: row.get("used_by"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    })
}

fn collab_task_from_row(row: sqlx::sqlite::SqliteRow) -> Result<CollabTask> {
    let status_text: String = row.get("status");
    let agent_type_text: Option<String> = row.get("assigned_agent_type");
    Ok(CollabTask {
        id: row.get("id"),
        project_id: row.get("project_id"),
        description: row.get("description"),
        assigned_agent_id: row.get("assigned_agent_id"),
        assigned_agent_type: agent_type_text
            .as_deref()
            .map(|t| parse_json_string_enum(t, "author type"))
            .transpose()?,
        status: parse_json_string_enum(&status_text, "collab task status")?,
        output: row.get("output"),
        requires_approval: row.get::<bool, _>("requires_approval"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        completed_at: row.get("completed_at"),
    })
}

fn collab_event_from_row(row: sqlx::sqlite::SqliteRow) -> Result<CollabEvent> {
    let event_type_text: String = row.get("event_type");
    let payload_json: String = row.get("payload_json");
    Ok(CollabEvent {
        id: row.get("id"),
        project_id: row.get("project_id"),
        event_type: parse_json_string_enum(&event_type_text, "collab event type")?,
        agent_id: row.get("agent_id"),
        payload: serde_json::from_str(&payload_json).unwrap_or_default(),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent1_core::{new_id, ApprovalRecord};
    use serde_json::json;
    use std::path::PathBuf;

    #[tokio::test]
    async fn approval_decision_round_trips() {
        let store = test_store("approval").await;
        let approval = ApprovalRecord {
            id: new_id("approval"),
            session_id: "sess_test".to_string(),
            agent_id: "assistant".to_string(),
            request: json!({"tool_name": "file_read"}),
            decision: None,
            decided_at: None,
            created_at: now(),
        };
        store
            .save_approval_request(&approval)
            .await
            .expect("save approval");
        store
            .update_approval_decision(&approval.id, "approved")
            .await
            .expect("update approval");
        let loaded = store
            .get_approval(&approval.id)
            .await
            .expect("get approval");
        assert_eq!(loaded.decision.as_deref(), Some("approved"));
        assert!(loaded.decided_at.is_some());
    }

    #[tokio::test]
    async fn agent_skill_search_finds_cards() {
        let store = test_store("skill").await;
        store
            .save_agent_card(&agent1_core::AgentCard {
                id: "critic".to_string(),
                name: "Critic".to_string(),
                description: Some("Reviews work".to_string()),
                skills: vec![agent1_core::AgentSkill {
                    name: "review".to_string(),
                    description: "Review plans and code".to_string(),
                }],
                input_modes: vec!["text".to_string()],
                output_modes: vec!["markdown".to_string()],
                endpoint: "http://127.0.0.1:17371/api/agents/critic/tasks".to_string(),
            })
            .await
            .expect("save card");
        let cards = store
            .find_agent_cards_by_skill("review")
            .await
            .expect("skill search");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "critic");
    }

    async fn test_store(name: &str) -> SqliteStore {
        let path = PathBuf::from("target").join(format!("agent1-db-{name}-{}.db", new_id("test")));
        SqliteStore::connect(path).await.expect("test sqlite store")
    }
}
