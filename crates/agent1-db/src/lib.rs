use std::{path::Path, str::FromStr};

use agent1_core::{
    Agent, Agent1Error, AgentCard, ApprovalRecord, EventType, McpServerConfig, MemoryItem, Message,
    MessageRole, Result, RuntimeEvent, Session, SessionStatus, ToolCallRecord, ToolCallStatus, now,
    redact_secrets_text, redact_secrets_value,
};
use chrono::{DateTime, Utc};
use sqlx::{Executor, Row, SqlitePool, sqlite::SqliteConnectOptions};

const INITIAL_SCHEMA: &str = include_str!("../migrations/0001_initial.sql");
const ORCHESTRATOR_SCHEMA: &str = include_str!("../migrations/0002_orchestrator.sql");

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
        self.pool
            .execute(INITIAL_SCHEMA)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to run initial migration: {err}")))?;
        self.pool
            .execute(ORCHESTRATOR_SCHEMA)
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to run orchestrator migration: {err}")))?;
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

    pub async fn create_session(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, title, root_agent_id, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&session.id)
        .bind(&session.title)
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
    ) -> Result<Session> {
        let created_at = now();
        let session = Session {
            id: agent1_core::new_id("sess"),
            title,
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
            SELECT id, title, root_agent_id, status, created_at, updated_at
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
            SELECT id, title, root_agent_id, status, created_at, updated_at
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent1_core::{ApprovalRecord, new_id};
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
