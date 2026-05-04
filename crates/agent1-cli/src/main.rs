use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
};

use agent1_core::{
    Agent, Agent1Error, AgentCard, AgentSkill, EventType, McpServerConfig, MemoryItem, ModelConfig,
    RuntimeEvent, SessionStatus, new_id, now,
};
use agent1_db::SqliteStore;
use agent1_models::provider_for;
use agent1_runtime::{
    AgentRuntime, ApprovalDelegate, ApprovalRequest, RiskLevel, RunAgentRequest, call_mcp_tool,
    list_mcp_tools, runtime_tool_definition, shutdown_mcp_pool,
};
use agent1_tools::ToolRegistry;
use async_trait::async_trait;
use axum::{
    Json, Router,
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
    routing::{delete, get, post},
};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    signal::ctrl_c,
    sync::{Mutex, oneshot},
    time::{Duration, sleep, timeout},
};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "agent1", version, about = "Local-first personal agent runtime")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        agent: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long)]
        auto_approve: bool,
    },
    Team {
        #[arg(long, default_value = "agents/planner.toml")]
        planner: PathBuf,
        #[arg(long, default_value = "agents/worker.toml")]
        worker: PathBuf,
        #[arg(long, default_value = "agents/critic.toml")]
        critic: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long)]
        auto_approve: bool,
    },
    Models {
        #[arg(long, default_value = "ollama")]
        provider: String,
        #[arg(long)]
        base_url: Option<String>,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    Server {
        #[arg(long, default_value = "127.0.0.1:17371")]
        bind: String,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long)]
        api_token: Option<String>,
    },
    Events {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Sessions {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Cancel {
        session: String,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    Approvals {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Export {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long, default_value = "markdown")]
        format: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Ui,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    Create {
        path: PathBuf,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    List {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    Cards {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long)]
        skill: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum MemoryCommand {
    Write {
        content: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, default_value = "agent")]
        scope: String,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long, default_value_t = 0)]
        importance: i32,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    Search {
        #[arg(default_value = "")]
        query: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    Delete {
        id: String,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum McpCommand {
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        command: String,
        #[arg(long)]
        arg: Vec<String>,
        #[arg(long, default_value_t = true)]
        enabled: bool,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    List {
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    Tools {
        server: String,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
    Call {
        server: String,
        tool: String,
        #[arg(long, default_value = "{}")]
        input: String,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run {
            agent,
            task,
            workspace,
            db,
            auto_approve,
        } => {
            let agent = load_agent(agent)?;
            let store = SqliteStore::connect(db).await?;
            let runtime = AgentRuntime::new(
                store,
                ToolRegistry::with_defaults(),
                CliApprovals { auto_approve },
            );
            let result = runtime
                .run(RunAgentRequest {
                    title: Some(task.chars().take(80).collect()),
                    agent,
                    input: task,
                    workspace_root: workspace,
                })
                .await?;
            println!("session: {}", result.session_id);
            println!();
            println!("{}", result.final_answer);
        }
        Command::Team {
            planner,
            worker,
            critic,
            task,
            workspace,
            db,
            auto_approve,
        } => {
            let store = SqliteStore::connect(db).await?;
            let plan = run_agent_once(
                store.clone(),
                load_agent(planner)?,
                format!("Create an execution plan for this task:\n\n{task}"),
                workspace.clone(),
                auto_approve,
            )
            .await?;
            println!("planner session: {}", plan.session_id);
            println!();
            println!("PLAN\n{}\n", plan.final_answer);

            let draft = run_agent_once(
                store.clone(),
                load_agent(worker)?,
                format!(
                    "Execute this task using the plan.\n\nTask:\n{task}\n\nPlan:\n{}",
                    plan.final_answer
                ),
                workspace.clone(),
                auto_approve,
            )
            .await?;
            println!("worker session: {}", draft.session_id);
            println!();
            println!("DRAFT\n{}\n", draft.final_answer);

            let review = run_agent_once(
                store,
                load_agent(critic)?,
                format!(
                    "Review the work for correctness, safety, missing steps, and final readiness.\n\nTask:\n{task}\n\nPlan:\n{}\n\nWorker output:\n{}",
                    plan.final_answer, draft.final_answer
                ),
                workspace,
                auto_approve,
            )
            .await?;
            println!("critic session: {}", review.session_id);
            println!();
            println!("CRITIC\n{}", review.final_answer);
        }
        Command::Models { provider, base_url } => {
            let config = ModelConfig {
                provider,
                model: "unused".to_string(),
                base_url,
                context_window: 8192,
                temperature: 0.2,
                top_p: None,
                max_tokens: None,
            };
            let provider = provider_for(&config)?;
            let models = provider.list_models(&config).await?;
            for model in models {
                println!("{}:{}", model.provider, model.name);
            }
        }
        Command::Agent { command } => match command {
            AgentCommand::Create { path, db } => {
                let agent = load_agent(path)?;
                let store = SqliteStore::connect(db).await?;
                store.save_agent(&agent).await?;
                store.save_agent_card(&card_for_agent(&agent)).await?;
                println!("saved agent {}", agent.id);
            }
            AgentCommand::List { db } => {
                let store = SqliteStore::connect(db).await?;
                for agent in store.list_agents().await? {
                    println!(
                        "{}\t{}\t{}",
                        agent.id,
                        agent.name,
                        agent.description.unwrap_or_default()
                    );
                }
            }
            AgentCommand::Cards { db, skill } => {
                let store = SqliteStore::connect(db).await?;
                let cards = if let Some(skill) = skill {
                    store.find_agent_cards_by_skill(&skill).await?
                } else {
                    store.list_agent_cards().await?
                };
                println!("{}", serde_json::to_string_pretty(&cards)?);
            }
        },
        Command::Memory { command } => match command {
            MemoryCommand::Write {
                content,
                agent,
                scope,
                tag,
                importance,
                db,
            } => {
                let store = SqliteStore::connect(db).await?;
                let timestamp = now();
                let item = MemoryItem {
                    id: new_id("mem"),
                    scope,
                    agent_id: agent,
                    content,
                    tags: tag,
                    embedding: None,
                    importance,
                    created_at: timestamp,
                    updated_at: timestamp,
                };
                store.write_memory(&item).await?;
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
            MemoryCommand::Search {
                query,
                agent,
                limit,
                db,
            } => {
                let store = SqliteStore::connect(db).await?;
                let memories = store
                    .search_memories(agent.as_deref(), &query, limit)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&memories)?);
            }
            MemoryCommand::Delete { id, db } => {
                let store = SqliteStore::connect(db).await?;
                store.delete_memory(&id).await?;
                println!("deleted {id}");
            }
        },
        Command::Mcp { command } => match command {
            McpCommand::Add {
                name,
                command,
                arg,
                enabled,
                db,
            } => {
                let store = SqliteStore::connect(db).await?;
                let timestamp = now();
                let server = McpServerConfig {
                    id: new_id("mcp"),
                    name,
                    transport: "stdio".to_string(),
                    command: Some(command),
                    args: arg,
                    env: Default::default(),
                    enabled,
                    created_at: timestamp,
                    updated_at: timestamp,
                };
                store.save_mcp_server(&server).await?;
                println!("{}", serde_json::to_string_pretty(&server)?);
            }
            McpCommand::List { db } => {
                let store = SqliteStore::connect(db).await?;
                let servers = store.list_mcp_servers().await?;
                println!("{}", serde_json::to_string_pretty(&servers)?);
            }
            McpCommand::Tools { server, db } => {
                let store = SqliteStore::connect(db).await?;
                let server = store.get_mcp_server(&server).await?;
                let tools = list_mcp_tools(&server).await?;
                println!("{}", serde_json::to_string_pretty(&tools)?);
            }
            McpCommand::Call {
                server,
                tool,
                input,
                db,
            } => {
                let store = SqliteStore::connect(db).await?;
                let server = store.get_mcp_server(&server).await?;
                let input: Value = serde_json::from_str(&input)?;
                let result = agent1_runtime::list_mcp_tools(&server).await?;
                let tool_names = result
                    .get("tools")
                    .and_then(Value::as_array)
                    .map(|tools| {
                        tools
                            .iter()
                            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !tool_names.is_empty() && !tool_names.iter().any(|name| *name == tool) {
                    return Err(anyhow::anyhow!(
                        "MCP tool `{tool}` was not listed by server `{}`",
                        server.name
                    ));
                }
                let result = call_mcp_tool(&server, &tool, input).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },
        Command::Server {
            bind,
            db,
            api_token,
        } => {
            run_server(bind, db, api_token).await?;
        }
        Command::Events { db, limit } => {
            let store = SqliteStore::connect(db).await?;
            let events = store.recent_events(limit).await?;
            for event in events {
                println!(
                    "{} {:?} agent={} session={} {}",
                    event.created_at,
                    event.event_type,
                    event.agent_id.unwrap_or_else(|| "-".to_string()),
                    event.session_id.unwrap_or_else(|| "-".to_string()),
                    event.payload
                );
            }
        }
        Command::Sessions { db, limit } => {
            let store = SqliteStore::connect(db).await?;
            let sessions = store.recent_sessions(limit).await?;
            for session in sessions {
                println!(
                    "{} {:?} agent={} title={}",
                    session.id,
                    session.status,
                    session.root_agent_id,
                    session.title.unwrap_or_else(|| "-".to_string())
                );
            }
        }
        Command::Cancel { session, db } => {
            let store = SqliteStore::connect(db).await?;
            store
                .update_session_status(&session, SessionStatus::Cancelled)
                .await?;
            store
                .save_event(&RuntimeEvent {
                    id: new_id("evt"),
                    session_id: Some(session.clone()),
                    agent_id: None,
                    event_type: EventType::RunCancelled,
                    payload: json!({"status": "cancelled"}),
                    created_at: now(),
                })
                .await?;
            println!("cancelled {session}");
        }
        Command::Approvals { db, limit } => {
            let store = SqliteStore::connect(db).await?;
            let approvals = store.recent_approvals(limit).await?;
            println!("{}", serde_json::to_string_pretty(&approvals)?);
        }
        Command::Export {
            db,
            session,
            format,
            output,
        } => {
            let store = SqliteStore::connect(db).await?;
            let session_record = store.get_session(&session).await?;
            let messages = store.session_messages(&session).await?;
            let events = store.session_events(&session).await?;
            let tool_calls = store.session_tool_calls(&session).await?;
            let rendered = match format.as_str() {
                "json" => serde_json::to_string_pretty(&json!({
                    "session": session_record,
                    "messages": messages,
                    "events": events,
                    "tool_calls": tool_calls
                }))?,
                "markdown" | "md" => {
                    render_session_markdown(&session_record, &messages, &events, &tool_calls)?
                }
                other => {
                    return Err(anyhow::anyhow!(
                        "unsupported export format `{other}`; use `markdown` or `json`"
                    ));
                }
            };
            if let Some(output) = output {
                tokio::fs::write(&output, rendered).await?;
                println!("wrote {}", output.display());
            } else {
                println!("{rendered}");
            }
        }
        Command::Ui => {
            println!("Agent1 Desktop");
            println!();
            println!("To use the desktop UI:");
            println!("  1. Start the API server:  agent1 server");
            println!(
                "  2. Launch the desktop app: npm run tauri:dev  (from the desktop/ directory)"
            );
            println!("     Or double-click the built Agent1 Desktop application.");
            println!();
            println!("The desktop app connects to http://127.0.0.1:17371 automatically.");
        }
    }
    Ok(())
}

async fn run_agent_once(
    store: SqliteStore,
    agent: Agent,
    input: String,
    workspace_root: PathBuf,
    auto_approve: bool,
) -> anyhow::Result<agent1_runtime::RunAgentResult> {
    let runtime = AgentRuntime::new(
        store,
        ToolRegistry::with_defaults(),
        CliApprovals { auto_approve },
    );
    Ok(runtime
        .run(RunAgentRequest {
            title: Some(input.chars().take(80).collect()),
            agent,
            input,
            workspace_root,
        })
        .await?)
}

fn load_agent(path: PathBuf) -> Result<Agent, Agent1Error> {
    let text = std::fs::read_to_string(&path).map_err(|err| {
        Agent1Error::Config(format!("failed to read `{}`: {err}", path.display()))
    })?;
    let agent = toml::from_str(&text).map_err(|err| {
        Agent1Error::Config(format!("failed to parse `{}`: {err}", path.display()))
    })?;
    validate_agent_tools(&agent)?;
    Ok(agent)
}

fn validate_agent_tools(agent: &Agent) -> Result<(), Agent1Error> {
    let registry = ToolRegistry::with_defaults();
    let unknown = agent
        .tools
        .iter()
        .find(|tool| registry.get(tool).is_none() && runtime_tool_definition(tool).is_none());
    if let Some(tool) = unknown {
        return Err(Agent1Error::Config(format!(
            "agent `{}` references unknown tool `{}`",
            agent.id, tool
        )));
    }
    Ok(())
}

fn card_for_agent(agent: &Agent) -> AgentCard {
    AgentCard {
        id: agent.id.clone(),
        name: agent.name.clone(),
        description: agent.description.clone(),
        skills: vec![AgentSkill {
            name: agent
                .role
                .clone()
                .unwrap_or_else(|| "general".to_string())
                .to_lowercase()
                .replace(' ', "_"),
            description: agent
                .description
                .clone()
                .unwrap_or_else(|| format!("Tasks handled by {}", agent.name)),
        }],
        input_modes: vec!["text".to_string()],
        output_modes: vec!["text".to_string(), "markdown".to_string()],
        endpoint: format!("http://127.0.0.1:17371/api/agents/{}/tasks", agent.id),
    }
}

#[derive(Clone)]
struct HttpState {
    store: SqliteStore,
    api_token: Option<String>,
    approval_broker: Arc<ApprovalBroker>,
}

#[derive(Default)]
struct ApprovalBroker {
    waiters: Mutex<BTreeMap<String, oneshot::Sender<String>>>,
}

impl ApprovalBroker {
    async fn wait_for(&self, approval_id: String) -> oneshot::Receiver<String> {
        let (sender, receiver) = oneshot::channel();
        self.waiters.lock().await.insert(approval_id, sender);
        receiver
    }

    async fn resolve(&self, approval_id: &str, decision: &str) {
        if let Some(sender) = self.waiters.lock().await.remove(approval_id) {
            let _ = sender.send(decision.to_string());
        }
    }

    async fn cancel(&self, approval_id: &str) {
        self.waiters.lock().await.remove(approval_id);
    }
}

#[derive(Debug, Serialize)]
struct ApiErrorEnvelope {
    error: BTreeMap<&'static str, String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "request_failed",
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        Self::internal(value.to_string())
    }
}

impl From<Agent1Error> for ApiError {
    fn from(value: Agent1Error) -> Self {
        match value {
            Agent1Error::Config(message)
            | Agent1Error::InvalidModelResponse(message)
            | Agent1Error::PathEscapesWorkspace(message) => Self::bad_request(message),
            Agent1Error::PermissionDenied(message) => Self {
                status: StatusCode::FORBIDDEN,
                code: "permission_denied",
                message,
            },
            Agent1Error::AgentNotFound(message) => Self {
                status: StatusCode::NOT_FOUND,
                code: "agent_not_found",
                message,
            },
            Agent1Error::ToolNotFound(message) => Self {
                status: StatusCode::NOT_FOUND,
                code: "tool_not_found",
                message,
            },
            Agent1Error::Runtime(message) => Self::internal(message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        let mut envelope = BTreeMap::new();
        envelope.insert("code", self.code.to_string());
        envelope.insert("message", self.message);
        let mut response =
            (self.status, Json(ApiErrorEnvelope { error: envelope })).into_response();
        apply_common_headers(&mut response);
        response
    }
}

fn apply_common_headers(response: &mut Response<Body>) {
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type, authorization"),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET,POST,OPTIONS"),
    );
}

fn json_ok(value: Value) -> Response<Body> {
    let mut response = Json(value).into_response();
    apply_common_headers(&mut response);
    response
}

fn require_auth(headers: &HeaderMap, state: &HttpState) -> Result<(), ApiError> {
    let Some(token) = state.api_token.as_ref() else {
        return Ok(());
    };
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let expected = format!("Bearer {token}");
    if auth == expected {
        Ok(())
    } else {
        Err(ApiError::unauthorized(
            "missing or invalid authorization token",
        ))
    }
}

async fn run_server(bind: String, db: PathBuf, api_token: Option<String>) -> anyhow::Result<()> {
    if !bind.starts_with("127.0.0.1:") && !bind.starts_with("localhost:") {
        return Err(anyhow::anyhow!(
            "server bind must be loopback for MVP; use 127.0.0.1:PORT"
        ));
    }
    let store = SqliteStore::connect(db).await?;
    let approval_broker = Arc::new(ApprovalBroker::default());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("Agent1 API listening on http://{bind}");

    let app = Router::new()
        .route("/", get(static_index))
        .route("/app", get(static_index))
        .route("/app/", get(static_index))
        .route("/api/agents", get(api_agents).post(api_agents_create))
        .route("/api/sessions", get(api_sessions).post(api_sessions_create))
        .route("/api/events", get(api_events))
        .route("/api/approvals", get(api_approvals))
        .route("/api/models", get(api_models))
        .route(
            "/api/mcp/servers",
            get(api_mcp_servers).post(api_mcp_servers_create),
        )
        .route(
            "/api/mcp/servers/{id}",
            delete(api_mcp_servers_delete).patch(api_mcp_servers_update),
        )
        .route("/api/mcp/servers/{id}/tools", get(api_mcp_servers_tools))
        .route("/api/mcp/servers/{id}/health", get(api_mcp_servers_health))
        .route("/api/memory", get(api_memory_search).post(api_memory_write))
        .route("/api/memory/{id}", delete(api_memory_delete))
        .route("/api/sessions/run", post(api_sessions_run))
        .route(
            "/api/sessions/{session_id}/run",
            post(api_sessions_run_for_id),
        )
        .route("/api/sessions/{session_id}/trace", get(api_session_trace))
        .route(
            "/api/sessions/{session_id}/cancel",
            post(api_session_cancel),
        )
        .route("/api/sessions/{session_id}/stream", get(api_session_stream))
        .route(
            "/api/tool-approvals/{approval_id}",
            post(api_tool_approval_decide),
        )
        .route("/api/agents/{agent_id}/tasks", post(api_agent_task))
        .route("/ws/events", get(ws_events))
        .route("/.well-known/agent.json", get(api_well_known_agent))
        .route("/api/health", get(api_health))
        .route("/{*path}", axum::routing::options(api_options))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(HttpState {
            store,
            api_token,
            approval_broker,
        });

    let server = axum::serve(listener, app);
    let server = server.with_graceful_shutdown(async {
        ctrl_c().await.expect("failed to listen for ctrl+c");
        println!("\nShutting down server...");
        shutdown_mcp_pool().await;
    });

    server.await?;
    Ok(())
}

async fn static_index() -> Result<Response<Body>, ApiError> {
    let body = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Agent1</title></head>
<body style="font-family:system-ui;background:#0a1118;color:#fefaf1;padding:2rem">
  <h1>Agent1 API Server</h1>
  <p>This server provides the Agent1 loopback API at <code>http://127.0.0.1:17371</code>.</p>
  <p>For the full desktop UI:</p>
  <ol>
    <li>Open a terminal in the <code>desktop/</code> directory</li>
    <li>Run <code>npm run tauri:dev</code></li>
    <li>Or run <code>npm run tauri:build</code> then launch the built application</li>
  </ol>
  <p>API docs: <a href="/api/health" style="color:#6cb9ff">/api/health</a></p>
</body>
</html>"#;
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    apply_common_headers(&mut response);
    Ok(response)
}

async fn api_options() -> Result<Response<Body>, ApiError> {
    Ok(json_ok(json!({"ok": true})))
}

async fn api_not_found() -> Result<Response<Body>, ApiError> {
    let mut response = (
        StatusCode::NOT_FOUND,
        Json(json!({"error":{"code":"not_found","message":"unknown endpoint"}})),
    )
        .into_response();
    apply_common_headers(&mut response);
    Ok(response)
}

async fn api_health() -> Result<Response<Body>, ApiError> {
    Ok(json_ok(json!({"ok": true, "service": "agent1-api"})))
}

async fn api_agents(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let agents = state.store.list_agents().await?;
    Ok(json_ok(json!({"agents": agents})))
}

async fn api_agents_create(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(agent): Json<Agent>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    state.store.save_agent(&agent).await?;
    state.store.save_agent_card(&card_for_agent(&agent)).await?;
    Ok(json_ok(json!({"agent": agent})))
}

async fn api_sessions(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let sessions = state.store.recent_sessions(50).await?;
    Ok(json_ok(json!({"sessions": sessions})))
}

async fn api_sessions_create(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let root_agent_id = body
        .get("root_agent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing root_agent_id"))?;
    let title = body
        .get("title")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let session = state
        .store
        .create_session_shell(root_agent_id, title)
        .await?;
    Ok(json_ok(
        json!({"session_id": session.id, "session": session}),
    ))
}

async fn api_events(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let events = state.store.recent_events(100).await?;
    Ok(json_ok(json!({"events": events})))
}

async fn api_approvals(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let approvals = state.store.recent_approvals(100).await?;
    Ok(json_ok(json!({"approvals": approvals})))
}

async fn api_models(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    Ok(json_ok(list_models_http().await?))
}

async fn api_mcp_servers(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let servers = state.store.list_mcp_servers().await?;
    Ok(json_ok(json!({"servers": servers})))
}

async fn api_mcp_servers_create(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing name"))?;
    let transport = body
        .get("transport")
        .and_then(Value::as_str)
        .unwrap_or("stdio");
    let command = body.get("command").and_then(Value::as_str);
    let args: Vec<String> = body
        .get("args")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let env: std::collections::BTreeMap<String, String> = body
        .get("env")
        .and_then(Value::as_object)
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let timestamp = now();
    let server = McpServerConfig {
        id: new_id("mcp"),
        name: name.to_string(),
        transport: transport.to_string(),
        command: command.map(String::from),
        args,
        env,
        enabled,
        created_at: timestamp,
        updated_at: timestamp,
    };
    state.store.save_mcp_server(&server).await?;
    Ok(json_ok(json!({"server": server})))
}

async fn api_mcp_servers_delete(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let _ = state.store.get_mcp_server(&id).await?;
    state.store.delete_mcp_server(&id).await?;
    Ok(json_ok(json!({"ok": true})))
}

async fn api_mcp_servers_update(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    if let Some(enabled) = body.get("enabled").and_then(Value::as_bool) {
        state.store.update_mcp_server_enabled(&id, enabled).await?;
    }
    Ok(json_ok(json!({"ok": true})))
}

async fn api_mcp_servers_tools(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let server = state.store.get_mcp_server(&id).await?;
    let tools = agent1_runtime::list_mcp_tools(&server).await?;
    Ok(json_ok(tools))
}

async fn api_mcp_servers_health(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let server = state.store.get_mcp_server(&id).await?;
    let healthy = agent1_runtime::check_mcp_server_health(&server).await;
    Ok(json_ok(json!({"healthy": healthy})))
}

async fn api_memory_search(
    State(state): State<HttpState>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let agent_id = params.get("agent").filter(|v| !v.is_empty());
    let query = params.get("query").map(String::from).unwrap_or_default();
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20);
    let memories = state
        .store
        .search_memories(agent_id.map(String::as_str), &query, limit)
        .await?;
    Ok(json_ok(
        json!({"memories": memories, "count": memories.len()}),
    ))
}

async fn api_memory_write(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing content"))?;
    let scope = body
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .to_string();
    let tags: Vec<String> = body
        .get("tags")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let importance = body.get("importance").and_then(Value::as_i64).unwrap_or(0) as i32;
    let agent_id = body.get("agent_id").and_then(Value::as_str);
    let timestamp = now();
    let item = MemoryItem {
        id: new_id("mem"),
        scope,
        agent_id: agent_id.map(String::from),
        content: content.to_string(),
        tags,
        embedding: None,
        importance,
        created_at: timestamp,
        updated_at: timestamp,
    };
    state.store.write_memory(&item).await?;
    Ok(json_ok(json!({"memory": item})))
}

async fn api_memory_delete(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    state.store.delete_memory(&id).await?;
    Ok(json_ok(json!({"ok": true})))
}

async fn api_well_known_agent(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let cards = state.store.list_agent_cards().await?;
    let value = cards
        .into_iter()
        .next()
        .map(|card| json!(card))
        .unwrap_or_else(|| json!({"error":{"message":"no agent cards saved"}}));
    Ok(json_ok(value))
}

async fn api_sessions_run(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    Ok(json_ok(
        run_agent_http(body, &state.store, state.approval_broker.clone()).await?,
    ))
}

async fn api_sessions_run_for_id(
    Path(session_id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(mut body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    if let Some(object) = body.as_object_mut() {
        object
            .entry("session_id".to_string())
            .or_insert(Value::String(session_id));
    }
    Ok(json_ok(
        run_agent_http(body, &state.store, state.approval_broker.clone()).await?,
    ))
}

async fn api_session_trace(
    Path(session_id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    Ok(json_ok(
        session_trace_http(&session_id, &state.store).await?,
    ))
}

async fn api_session_cancel(
    Path(session_id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    Ok(json_ok(
        cancel_session_http(&session_id, &state.store).await?,
    ))
}

async fn api_session_stream(
    Path(session_id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let events = state
        .store
        .session_events(&session_id)
        .await
        .unwrap_or_default();

    let mut output = String::new();
    output.push_str("event: iteration_start\ndata: {}\n\n");

    for event in events.iter() {
        let event_type = match event.event_type {
            agent1_core::EventType::SessionStarted => "session_start",
            agent1_core::EventType::MemoryRead => "memory_read",
            agent1_core::EventType::MemoryWritten => "memory_written",
            agent1_core::EventType::ModelCallStarted => "model_call_start",
            agent1_core::EventType::ModelOutputDelta => "chunk",
            agent1_core::EventType::ModelCallCompleted => "model_call_end",
            agent1_core::EventType::ToolCallStarted => "tool_call_start",
            agent1_core::EventType::ToolCallCompleted => "tool_call_end",
            agent1_core::EventType::ToolCallFailed => "tool_call_failed",
            agent1_core::EventType::RunCancelled => "cancelled",
            agent1_core::EventType::FinalAnswer => "final",
            _ => "event",
        };
        let payload = serde_json::to_string(&event.payload).unwrap_or_else(|_| "{}".to_string());
        output.push_str(&format!("event: {}\ndata: {}\n\n", event_type, payload));
    }

    output.push_str("event: final\ndata: \"done\"\n\n");

    let mut response = Response::new(Body::from(output));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    apply_common_headers(&mut response);
    Ok(response)
}

async fn api_tool_approval_decide(
    Path(approval_id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let decision = body
        .get("decision")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing decision"))?;
    apply_approval_decision(&state.store, &state.approval_broker, &approval_id, decision).await?;
    Ok(json_ok(
        json!({"approval_id": approval_id, "decision": decision}),
    ))
}

async fn api_agent_task(
    Path(agent_id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    Ok(json_ok(
        run_agent_task_http(&agent_id, body, &state.store, state.approval_broker.clone()).await?,
    ))
}

async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    require_auth(&headers, &state)?;
    let store = state.store.clone();
    let approval_broker = state.approval_broker.clone();
    Ok(ws.on_upgrade(move |socket| ws_events_stream(socket, store, approval_broker)))
}

async fn ws_events_stream(
    mut socket: WebSocket,
    store: SqliteStore,
    approval_broker: Arc<ApprovalBroker>,
) {
    let mut seen_ids = BTreeSet::new();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else {
                    return;
                };
                match message {
                    Message::Text(text) => {
                        match handle_ws_client_message(&text, &store, &approval_broker).await {
                            Ok(ack) => {
                                if let Some(ack) = ack {
                                    if socket.send(Message::Text(ack.into())).await.is_err() {
                                        return;
                                    }
                                }
                            }
                            Err(err) => {
                                let _ = socket.send(
                                    Message::Text(
                                        json!({"type":"error","message": err.to_string()}).to_string().into()
                                    )
                                ).await;
                            }
                        }
                    }
                    Message::Close(_) => return,
                    _ => {}
                }
            }
            _ = sleep(Duration::from_millis(500)) => {
                if send_recent_events(&mut socket, &store, &mut seen_ids).await.is_err() {
                    return;
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApprovalDecisionMessage {
    #[serde(rename = "type")]
    message_type: String,
    approval_id: String,
    decision: String,
}

async fn handle_ws_client_message(
    text: &str,
    store: &SqliteStore,
    approval_broker: &ApprovalBroker,
) -> anyhow::Result<Option<String>> {
    let message: ApprovalDecisionMessage = serde_json::from_str(text)?;
    if message.message_type != "approval_decision" {
        return Ok(None);
    }
    apply_approval_decision(
        store,
        approval_broker,
        &message.approval_id,
        &message.decision,
    )
    .await?;
    Ok(Some(
        json!({
            "type": "approval_decision_ack",
            "approval_id": message.approval_id,
            "decision": message.decision
        })
        .to_string(),
    ))
}

async fn send_recent_events(
    socket: &mut WebSocket,
    store: &SqliteStore,
    seen_ids: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    let events = store.recent_events(200).await?;
    for event in events.into_iter().rev() {
        if !seen_ids.insert(event.id.clone()) {
            continue;
        }
        let payload = json!({"type":"event","event": event}).to_string();
        socket.send(Message::Text(payload.into())).await?;
    }
    Ok(())
}

async fn apply_approval_decision(
    store: &SqliteStore,
    approval_broker: &ApprovalBroker,
    approval_id: &str,
    decision: &str,
) -> Result<(), Agent1Error> {
    let _ = parse_approval_decision(decision)?;
    store
        .update_approval_decision(approval_id, decision)
        .await?;
    approval_broker.resolve(approval_id, decision).await;
    Ok(())
}

async fn session_trace_http(session_id: &str, store: &SqliteStore) -> anyhow::Result<Value> {
    let session = store.get_session(session_id).await?;
    let messages = store.session_messages(session_id).await?;
    let tool_calls = store.session_tool_calls(session_id).await?;
    let events = store.session_events(session_id).await?;
    let approvals = store.recent_approvals(200).await?;
    let approvals = approvals
        .into_iter()
        .filter(|approval| approval.session_id == session_id)
        .collect::<Vec<_>>();
    Ok(json!({
        "session": session,
        "messages": messages,
        "tool_calls": tool_calls,
        "events": events,
        "approvals": approvals
    }))
}

async fn list_models_http() -> anyhow::Result<Value> {
    let configs = [ModelConfig {
        provider: "ollama".to_string(),
        model: "unused".to_string(),
        base_url: None,
        context_window: 8192,
        temperature: 0.2,
        top_p: None,
        max_tokens: None,
    }];
    let mut providers = Vec::new();
    for config in configs {
        let result = match provider_for(&config) {
            Ok(provider) => match provider.list_models(&config).await {
                Ok(models) => json!({"provider": config.provider, "models": models}),
                Err(err) => json!({"provider": config.provider, "error": err.to_string()}),
            },
            Err(err) => json!({"provider": config.provider, "error": err.to_string()}),
        };
        providers.push(result);
    }
    Ok(json!({"providers": providers}))
}

async fn cancel_session_http(session_id: &str, store: &SqliteStore) -> anyhow::Result<Value> {
    store
        .update_session_status(session_id, SessionStatus::Cancelled)
        .await?;
    store
        .save_event(&RuntimeEvent {
            id: new_id("evt"),
            session_id: Some(session_id.to_string()),
            agent_id: None,
            event_type: EventType::RunCancelled,
            payload: json!({"status": "cancelled"}),
            created_at: now(),
        })
        .await?;
    Ok(json!({"session_id": session_id, "status": "cancelled"}))
}

async fn run_agent_task_http(
    agent_id: &str,
    mut body: Value,
    store: &SqliteStore,
    approval_broker: Arc<ApprovalBroker>,
) -> anyhow::Result<Value> {
    if let Some(object) = body.as_object_mut() {
        object.insert("agent_id".to_string(), Value::String(agent_id.to_string()));
    }
    run_agent_http(body, store, approval_broker).await
}

async fn run_agent_http(
    body: Value,
    store: &SqliteStore,
    approval_broker: Arc<ApprovalBroker>,
) -> anyhow::Result<Value> {
    let agent_id = body
        .get("agent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let input = body
        .get("input")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing input"))?;
    let workspace = body.get("workspace").and_then(Value::as_str).unwrap_or(".");
    let agent = store.get_agent(agent_id).await?;
    let runtime = AgentRuntime::new(
        store.clone(),
        ToolRegistry::with_defaults(),
        ServerApprovals {
            store: store.clone(),
            timeout: Duration::from_secs(120),
            approval_broker,
        },
    );
    let result = runtime
        .run(RunAgentRequest {
            title: Some(input.chars().take(80).collect()),
            agent,
            input: input.to_string(),
            workspace_root: PathBuf::from(workspace),
        })
        .await?;
    Ok(json!({"session_id": result.session_id, "final": result.final_answer}))
}

#[derive(Clone)]
struct CliApprovals {
    auto_approve: bool,
}

#[derive(Clone)]
struct ServerApprovals {
    store: SqliteStore,
    timeout: Duration,
    approval_broker: Arc<ApprovalBroker>,
}

#[async_trait]
impl ApprovalDelegate for ServerApprovals {
    async fn approve(&self, request: ApprovalRequest) -> Result<bool, Agent1Error> {
        println!(
            "waiting for API approval: approval={} agent={} tool={}",
            request.approval_id, request.agent_id, request.tool_name
        );
        let approval = self.store.get_approval(&request.approval_id).await?;
        if let Some(decision) = approval.decision.as_deref() {
            return parse_approval_decision(decision);
        }

        let receiver = self
            .approval_broker
            .wait_for(request.approval_id.clone())
            .await;

        match timeout(self.timeout, receiver).await {
            Ok(Ok(decision)) => parse_approval_decision(&decision),
            Ok(Err(_)) => {
                let approval = self.store.get_approval(&request.approval_id).await?;
                match approval.decision.as_deref() {
                    Some(decision) => parse_approval_decision(decision),
                    None => Err(Agent1Error::Runtime(
                        "approval channel closed before a decision was recorded".to_string(),
                    )),
                }
            }
            Err(_) => {
                self.approval_broker.cancel(&request.approval_id).await;
                self.store
                    .update_approval_decision(&request.approval_id, "denied")
                    .await?;
                Ok(false)
            }
        }
    }
}

fn parse_approval_decision(decision: &str) -> Result<bool, Agent1Error> {
    match decision {
        "approved" | "always_allow_for_session" => Ok(true),
        "denied" => Ok(false),
        other => Err(Agent1Error::Config(format!(
            "unsupported approval decision `{other}`"
        ))),
    }
}

#[async_trait]
impl ApprovalDelegate for CliApprovals {
    async fn approve(&self, request: ApprovalRequest) -> Result<bool, Agent1Error> {
        if self.auto_approve {
            println!("auto-approved tool: {}", request.tool_name);
            return Ok(true);
        }
        println!();
        println!("Tool approval requested");
        println!("agent: {}", request.agent_id);
        println!("session: {}", request.session_id);
        println!("tool: {}", request.tool_name);
        println!("risk: {}", risk_label(request.risk));
        println!("input:");
        println!(
            "{}",
            serde_json::to_string_pretty(&request.input)
                .unwrap_or_else(|_| request.input.to_string())
        );
        print!("Approve? [y/N] ");
        io::stdout()
            .flush()
            .map_err(|err| Agent1Error::Runtime(format!("failed to flush stdout: {err}")))?;
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|err| Agent1Error::Runtime(format!("failed to read approval: {err}")))?;
        Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
    }
}

fn risk_label(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
    }
}

fn render_session_markdown(
    session: &agent1_core::Session,
    messages: &[agent1_core::Message],
    events: &[agent1_core::RuntimeEvent],
    tool_calls: &[agent1_core::ToolCallRecord],
) -> anyhow::Result<String> {
    let mut out = String::new();
    out.push_str(&format!("# Agent1 Session {}\n\n", session.id));
    out.push_str(&format!("- Status: {:?}\n", session.status));
    out.push_str(&format!("- Root agent: {}\n", session.root_agent_id));
    out.push_str(&format!("- Created: {}\n", session.created_at));
    if let Some(title) = &session.title {
        out.push_str(&format!("- Title: {title}\n"));
    }
    out.push_str("\n## Messages\n\n");
    for message in messages {
        out.push_str(&format!(
            "### {:?} {}\n\n{}\n\n",
            message.role,
            message.from_agent_id.as_deref().unwrap_or("user"),
            fence_if_needed(&message.content)
        ));
    }
    out.push_str("## Tool Calls\n\n");
    if tool_calls.is_empty() {
        out.push_str("No tool calls recorded.\n\n");
    } else {
        for call in tool_calls {
            out.push_str(&format!(
                "### {} {:?}\n\nInput:\n```json\n{}\n```\n\n",
                call.tool_name,
                call.status,
                serde_json::to_string_pretty(&call.input)?
            ));
            if let Some(output) = &call.output {
                out.push_str(&format!(
                    "Output:\n```json\n{}\n```\n\n",
                    serde_json::to_string_pretty(output)?
                ));
            }
            if let Some(error) = &call.error {
                out.push_str(&format!("Error: {error}\n\n"));
            }
        }
    }
    out.push_str("## Events\n\n");
    for event in events {
        out.push_str(&format!(
            "- `{}` {:?} agent={} payload=`{}`\n",
            event.created_at,
            event.event_type,
            event.agent_id.as_deref().unwrap_or("-"),
            event.payload
        ));
    }
    Ok(out)
}

fn fence_if_needed(content: &str) -> String {
    if content.contains('\n') || content.contains('{') {
        format!("```text\n{content}\n```")
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_agent_toml(extra: &str) -> String {
        format!(
            r#"
id = "assistant"
name = "Assistant"
system_prompt = "Help."
tools = ["file_read"]

[model]
provider = "mock"
model = "final"

[permissions]
file_read = "ask"
{extra}
"#
        )
    }

    #[test]
    fn missing_name_fails() {
        let text = valid_agent_toml("").replace("name = \"Assistant\"\n", "");
        let result = toml::from_str::<Agent>(&text);
        assert!(result.is_err());
    }

    #[test]
    fn missing_model_fails() {
        let text = r#"
id = "assistant"
name = "Assistant"
system_prompt = "Help."
"#;
        let result = toml::from_str::<Agent>(text);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_permission_value_fails() {
        let text = valid_agent_toml("shell = \"sometimes\"");
        let result = toml::from_str::<Agent>(&text);
        assert!(result.is_err());
    }

    #[test]
    fn unknown_tool_fails_with_useful_error() {
        let text = valid_agent_toml("").replace("tools = [\"file_read\"]", "tools = [\"nope\"]");
        let agent = toml::from_str::<Agent>(&text).expect("agent parses before validation");
        let error = validate_agent_tools(&agent).expect_err("unknown tool should fail");
        assert!(error.to_string().contains("unknown tool `nope`"));
    }
}
