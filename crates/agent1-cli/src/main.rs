use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Command as StdCommand, Stdio},
    sync::Arc,
};

use agent1_collab::CollaborationEngine;
use agent1_core::{
    new_id, now, Agent, Agent1Error, AgentCard, AgentSkill, EventType, McpServerConfig, MemoryItem,
    ModelConfig, RuntimeEvent, SessionStatus,
};
use agent1_core::{AuthorType, CollaborationMode, ExternalPermissions};
use agent1_db::SqliteStore;
use agent1_gateway::{ExternalGateway, GatewayMessage};
use agent1_models::provider_for;
use agent1_orchestrator::{run_orchestration, ProgressTracker};
use agent1_runtime::{
    call_mcp_tool, list_mcp_tools, runtime_tool_definition, shutdown_mcp_pool, shutdown_mcp_scope,
    AgentRuntime, ApprovalDelegate, ApprovalRequest, RiskLevel, RunAgentRequest,
};
use agent1_tools::ToolRegistry;
use agent1_whatsapp::WhatsAppService;
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, Method, Response, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    signal::ctrl_c,
    sync::{oneshot, Mutex},
    task::AbortHandle,
    time::{sleep, timeout, Duration},
};
use tower_http::cors::{Any, CorsLayer};
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
    Loop {
        #[arg(long)]
        task: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long, default_value = ".agent1/agent1.db")]
        db: PathBuf,
        #[arg(long)]
        auto_approve: bool,
        #[arg(long, default_value_t = 5)]
        max_runs: usize,
        #[arg(long, default_value = "AGENT1_LOOP_COMPLETE")]
        completion_signal: String,
        #[arg(long, default_value_t = 2)]
        completion_threshold: usize,
        #[arg(long)]
        notes: Option<PathBuf>,
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
                    project_id: None,
                    workspace_root: workspace,
                    session_id: None,
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
        Command::Loop {
            task,
            workspace,
            db,
            auto_approve,
            max_runs,
            completion_signal,
            completion_threshold,
            notes,
        } => {
            run_autonomous_loop(
                task,
                workspace,
                db,
                auto_approve,
                max_runs,
                completion_signal,
                completion_threshold,
                notes,
            )
            .await?;
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
            cancel_session_http(&session, &store).await?;
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
            project_id: None,
            workspace_root,
            session_id: None,
        })
        .await?)
}

async fn run_autonomous_loop(
    task: String,
    workspace: PathBuf,
    db: PathBuf,
    auto_approve: bool,
    max_runs: usize,
    completion_signal: String,
    completion_threshold: usize,
    notes: Option<PathBuf>,
) -> anyhow::Result<()> {
    if max_runs == 0 {
        return Err(anyhow::anyhow!("--max-runs must be greater than zero"));
    }
    if completion_threshold == 0 {
        return Err(anyhow::anyhow!(
            "--completion-threshold must be greater than zero"
        ));
    }

    let notes_path = autonomous_notes_path(&workspace, notes);
    if let Some(parent) = notes_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut consecutive_complete = 0_usize;

    println!("autonomous loop notes: {}", notes_path.display());
    println!("completion signal: {completion_signal}");
    println!("max runs: {max_runs}");
    println!();

    for iteration in 1..=max_runs {
        let notes_text = tokio::fs::read_to_string(&notes_path)
            .await
            .unwrap_or_else(|_| initial_loop_notes(&task));
        let objective =
            build_loop_objective(&task, iteration, max_runs, &completion_signal, &notes_text);

        println!("loop iteration {iteration}/{max_runs}");
        let response = run_orchestration(
            &db,
            &objective,
            Some(workspace.to_string_lossy().to_string()),
            auto_approve,
        )
        .await?;
        println!(
            "orchestration={} plan={} status={}",
            response.orchestration_id, response.plan_id, response.status
        );

        let store = SqliteStore::connect(&db).await?;
        let tracker = ProgressTracker::new(store);
        let (_plan, steps) = tracker.get_plan_with_steps(&response.plan_id).await?;
        let iteration_summary = render_loop_iteration_summary(iteration, &response, &steps);
        append_loop_notes(&notes_path, &iteration_summary).await?;

        let contains_signal = completion_seen(
            &completion_signal,
            &[response.message.as_str(), iteration_summary.as_str()],
        );
        if contains_signal {
            consecutive_complete += 1;
        } else {
            consecutive_complete = 0;
        }

        if response.status != "Completed" {
            println!("stopping: iteration status was {}", response.status);
            break;
        }
        if consecutive_complete >= completion_threshold {
            println!("stopping: completion signal seen {consecutive_complete} consecutive time(s)");
            break;
        }
        if iteration == max_runs {
            println!("stopping: max runs reached");
        }
    }

    Ok(())
}

fn autonomous_notes_path(workspace: &std::path::Path, notes: Option<PathBuf>) -> PathBuf {
    match notes {
        Some(path) if path.is_absolute() => path,
        Some(path) => workspace.join(path),
        None => workspace.join(".agent1").join("autonomous-loop-notes.md"),
    }
}

fn initial_loop_notes(task: &str) -> String {
    format!(
        "# Agent1 Autonomous Loop Notes\n\n## Objective\n{task}\n\n## Progress\n- No prior loop iterations.\n"
    )
}

fn build_loop_objective(
    task: &str,
    iteration: usize,
    max_runs: usize,
    completion_signal: &str,
    notes: &str,
) -> String {
    format!(
        r#"Autonomous loop iteration {iteration}/{max_runs}.

Primary objective:
{task}

Persistent notes from previous iterations:
{notes}

Continue the objective from the notes. Do the next concrete useful work, verify it, and update durable knowledge through memory when it will help future runs.

If the objective is fully complete and verified, include this exact completion signal in a final worker, critic, or reporter output:
{completion_signal}

If the objective is not complete, do not include the completion signal. Report remaining work clearly."#
    )
}

fn render_loop_iteration_summary(
    iteration: usize,
    response: &agent1_orchestrator::OrchestrateResponse,
    steps: &[agent1_core::ExecutionStep],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n## Iteration {iteration}\n- Orchestration: {}\n- Plan: {}\n- Status: {}\n- Message: {}\n\n",
        response.orchestration_id, response.plan_id, response.status, response.message
    ));
    for step in steps {
        out.push_str(&format!(
            "### Step {} {:?}\n{}\n\n",
            step.step_order + 1,
            step.status,
            step.description
        ));
        if let Some(output) = &step.output {
            out.push_str("Output:\n");
            out.push_str(&truncate_notes_output(output, 2_000));
            out.push_str("\n\n");
        }
    }
    out
}

fn truncate_notes_output(output: &str, max_chars: usize) -> String {
    let mut text = output.chars().take(max_chars).collect::<String>();
    if output.chars().count() > max_chars {
        text.push_str("\n[truncated]");
    }
    text
}

fn completion_seen(signal: &str, texts: &[&str]) -> bool {
    !signal.trim().is_empty() && texts.iter().any(|text| text.contains(signal))
}

async fn append_loop_notes(path: &PathBuf, text: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(text.as_bytes()).await?;
    Ok(())
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
    active_runs: Arc<ActiveRunRegistry>,
    whatsapp: Arc<WhatsAppService>,
    collab_engine: Arc<CollaborationEngine>,
    gateway: Arc<ExternalGateway>,
}

#[derive(Default)]
struct ActiveRunRegistry {
    handles: Mutex<BTreeMap<String, AbortHandle>>,
}

impl ActiveRunRegistry {
    async fn insert(&self, session_id: String, handle: AbortHandle) {
        self.handles.lock().await.insert(session_id, handle);
    }

    async fn abort(&self, session_id: &str) -> bool {
        let handle = self.handles.lock().await.get(session_id).cloned();
        if let Some(handle) = handle {
            handle.abort();
            true
        } else {
            false
        }
    }

    async fn remove(&self, session_id: &str) {
        self.handles.lock().await.remove(session_id);
    }
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
        HeaderValue::from_static("GET,POST,PATCH,DELETE,OPTIONS"),
    );
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
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
    reconcile_interrupted_sessions(&store).await?;
    let approval_broker = Arc::new(ApprovalBroker::default());
    let active_runs = Arc::new(ActiveRunRegistry::default());
    let collab_engine = Arc::new(CollaborationEngine::new(store.clone()));
    let gateway = Arc::new(ExternalGateway::new(collab_engine.clone()));

    // Start gateway cleanup task
    let gw_clone = gateway.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            gw_clone.cleanup_stale_connections().await;
        }
    });

    if let Err(err) = bootstrap_whatsapp_sidecar() {
        eprintln!("WhatsApp sidecar bootstrap skipped: {err}");
    }
    let whatsapp_service = Arc::new(WhatsAppService::new());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("Agent1 API listening on http://{bind}");

    let app = Router::new()
        .route("/", get(static_index))
        .route("/app", get(static_index))
        .route("/app/", get(static_index))
        .route("/api/agents", get(api_agents).post(api_agents_create))
        .route("/api/agents/{agent_id}", delete(api_agents_delete))
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
        .route("/api/whatsapp/status", get(api_whatsapp_status))
        .route("/api/whatsapp/connect", post(api_whatsapp_connect))
        .route("/api/whatsapp/disconnect", post(api_whatsapp_disconnect))
        .route("/api/whatsapp/reset", post(api_whatsapp_reset))
        .route("/api/whatsapp/qr", get(api_whatsapp_qr))
        .route("/api/whatsapp/send", post(api_whatsapp_send))
        .route("/{*path}", axum::routing::options(api_options))
        // --- Collaboration Workspace Routes ---
        .route(
            "/api/projects",
            get(api_projects_list).post(api_projects_create),
        )
        .route(
            "/api/projects/{id}",
            get(api_projects_get)
                .patch(api_projects_update)
                .delete(api_projects_delete),
        )
        .route("/api/projects/{id}/agents", post(api_projects_add_agent))
        .route(
            "/api/projects/{id}/agents/{agent_id}",
            delete(api_projects_remove_agent),
        )
        .route("/api/projects/{id}/invite", post(api_projects_invite))
        .route("/api/projects/{id}/invites", get(api_projects_invites))
        .route(
            "/api/projects/{id}/invites/{token}",
            delete(api_projects_invite_revoke),
        )
        .route("/api/projects/{id}/externals", get(api_projects_externals))
        .route(
            "/api/projects/{id}/externals/{ext_id}",
            delete(api_projects_externals_remove),
        )
        .route(
            "/api/projects/{id}/blackboard",
            get(api_projects_blackboard).post(api_projects_blackboard_write),
        )
        .route(
            "/api/projects/{id}/tasks",
            get(api_projects_tasks).post(api_projects_tasks_submit),
        )
        .route("/gateway/connect", get(ws_gateway_connect))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(cors_layer())
        .with_state(HttpState {
            store,
            api_token,
            approval_broker,
            active_runs,
            whatsapp: whatsapp_service,
            collab_engine,
            gateway,
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

fn bootstrap_whatsapp_sidecar() -> anyhow::Result<()> {
    if TcpStream::connect("127.0.0.1:17372").is_ok() {
        return Ok(());
    }

    let sidecar_dir = std::env::current_dir()?.join("whatsapp-sidecar");
    if !sidecar_dir.join("package.json").exists() {
        anyhow::bail!(
            "whatsapp-sidecar/package.json not found from {}",
            std::env::current_dir()?.display()
        );
    }

    if !sidecar_dir.join("node_modules").exists() {
        println!("Installing WhatsApp sidecar dependencies...");
        let status = StdCommand::new(npm_command())
            .arg("install")
            .current_dir(&sidecar_dir)
            .status()?;
        if !status.success() {
            anyhow::bail!("npm install failed for WhatsApp sidecar");
        }
    }

    println!("Starting WhatsApp sidecar on http://127.0.0.1:17372");
    StdCommand::new(npm_command())
        .arg("start")
        .current_dir(sidecar_dir)
        .env("PORT", "17372")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn npm_command() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
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

async fn api_whatsapp_status(State(state): State<HttpState>) -> Result<Response<Body>, ApiError> {
    let status = state
        .whatsapp
        .get_status()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(
        serde_json::to_value(status).map_err(|e| ApiError::internal(e.to_string()))?,
    ))
}

async fn api_whatsapp_connect(State(state): State<HttpState>) -> Result<Response<Body>, ApiError> {
    state
        .whatsapp
        .connect()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(json!({"status": "connecting"})))
}

async fn api_whatsapp_disconnect(
    State(state): State<HttpState>,
) -> Result<Response<Body>, ApiError> {
    state
        .whatsapp
        .disconnect()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(json!({"status": "disconnected"})))
}

async fn api_whatsapp_reset(State(state): State<HttpState>) -> Result<Response<Body>, ApiError> {
    state
        .whatsapp
        .reset()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(json!({"status": "reset"})))
}

async fn api_whatsapp_qr(State(state): State<HttpState>) -> Result<Response<Body>, ApiError> {
    match state.whatsapp.get_qr_svg().await {
        Ok(Some(svg)) => {
            let mut response = Response::new(Body::from(svg));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("image/svg+xml"),
            );
            Ok(response)
        }
        Ok(None) => Ok(json_ok(json!({"qr": null, "message": "no qr available"}))),
        Err(e) => Err(ApiError::internal(e.to_string())),
    }
}

async fn api_whatsapp_send(
    State(state): State<HttpState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, ApiError> {
    let to = payload["to"]
        .as_str()
        .ok_or_else(|| ApiError::bad_request("missing 'to' field"))?;
    let text = payload["text"]
        .as_str()
        .ok_or_else(|| ApiError::bad_request("missing 'text' field"))?;

    match state.whatsapp.send_message(to, text).await {
        Ok(result) => Ok(json_ok(
            serde_json::to_value(result).map_err(|e| ApiError::internal(e.to_string()))?,
        )),
        Err(e) => Err(ApiError::internal(e.to_string())),
    }
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

async fn api_agents_delete(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    if agent_id.eq_ignore_ascii_case("agent1") {
        return Err(ApiError::bad_request("Agent1 cannot be deleted"));
    }
    state.store.delete_agent(&agent_id).await?;
    Ok(json_ok(json!({"deleted": agent_id})))
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
    let project_id = body
        .get("project_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    let session = state
        .store
        .create_session_shell(root_agent_id, title, project_id)
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
    if let Some(project_id) = body
        .get("project_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let project = state.store.get_project(project_id).await?;
        if project.collaboration_mode == CollaborationMode::Airgapped {
            return Err(ApiError::bad_request(
                "Airgapped policy blocks MCP server changes",
            ));
        }
    }
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
    let existing_servers = state.store.list_mcp_servers().await?;
    if existing_servers.iter().any(|server| {
        server.name.eq_ignore_ascii_case(name)
            && server.transport == transport
            && server.command.as_deref() == command
            && server.args == args
    }) {
        return Err(ApiError::bad_request(
            "MCP server with the same name, transport, command, and args already exists",
        ));
    }
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
        run_agent_http(
            body,
            &state.store,
            state.approval_broker.clone(),
            state.active_runs.clone(),
        )
        .await?,
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
        run_agent_http(
            body,
            &state.store,
            state.approval_broker.clone(),
            state.active_runs.clone(),
        )
        .await?,
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
        cancel_session_with_abort(&session_id, &state.store, state.active_runs.clone()).await?,
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
    let configs = [
        ModelConfig {
            provider: "opencode".to_string(),
            model: "unused".to_string(),
            base_url: None,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        },
        ModelConfig {
            provider: "ollama".to_string(),
            model: "unused".to_string(),
            base_url: None,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        },
        ModelConfig {
            provider: "openai_compatible".to_string(),
            model: "unused".to_string(),
            base_url: None,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        },
        ModelConfig {
            provider: "nvidia".to_string(),
            model: "unused".to_string(),
            base_url: None,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        },
    ];
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
    mark_session_cancelled(session_id, store, false).await
}

async fn cancel_session_with_abort(
    session_id: &str,
    store: &SqliteStore,
    active_runs: Arc<ActiveRunRegistry>,
) -> anyhow::Result<Value> {
    let aborted = active_runs.abort(session_id).await;
    shutdown_mcp_scope(session_id).await;
    mark_session_cancelled(session_id, store, aborted).await
}

async fn mark_session_cancelled(
    session_id: &str,
    store: &SqliteStore,
    aborted: bool,
) -> anyhow::Result<Value> {
    store
        .update_session_status(session_id, SessionStatus::Cancelled)
        .await?;
    store
        .save_event(&RuntimeEvent {
            id: new_id("evt"),
            session_id: Some(session_id.to_string()),
            agent_id: None,
            event_type: EventType::RunCancelled,
            payload: json!({"status": "cancelled", "aborted": aborted}),
            created_at: now(),
        })
        .await?;
    Ok(json!({"session_id": session_id, "status": "cancelled", "aborted": aborted}))
}

async fn reconcile_interrupted_sessions(store: &SqliteStore) -> anyhow::Result<()> {
    let sessions = store.recent_sessions(1000).await?;
    for session in sessions
        .into_iter()
        .filter(|session| session.status == SessionStatus::Running)
    {
        store
            .update_session_status(&session.id, SessionStatus::Failed)
            .await?;
        store
            .save_event(&RuntimeEvent {
                id: new_id("evt"),
                session_id: Some(session.id.clone()),
                agent_id: Some(session.root_agent_id.clone()),
                event_type: EventType::Error,
                payload: json!({
                    "message": "session was interrupted before the API server restarted"
                }),
                created_at: now(),
            })
            .await?;
    }
    Ok(())
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
    let active_runs = Arc::new(ActiveRunRegistry::default());
    run_agent_http(body, store, approval_broker, active_runs).await
}

async fn run_agent_http(
    body: Value,
    store: &SqliteStore,
    approval_broker: Arc<ApprovalBroker>,
    active_runs: Arc<ActiveRunRegistry>,
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
    let project_id = body
        .get("project_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(String::from);
    let agent = store.get_agent(agent_id).await?;
    let session_id = body
        .get("session_id")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| new_id("sess"));
    let store_for_task = store.clone();
    let approval_broker_for_task = approval_broker.clone();
    let input = input.to_string();
    let workspace_root = PathBuf::from(workspace);
    let task_session_id = session_id.clone();
    let task_project_id = project_id.clone();
    let task = tokio::spawn(async move {
        let runtime = AgentRuntime::new(
            store_for_task.clone(),
            ToolRegistry::with_defaults(),
            ServerApprovals {
                store: store_for_task.clone(),
                timeout: Duration::from_secs(120),
                approval_broker: approval_broker_for_task,
            },
        );
        let result = runtime
            .run(RunAgentRequest {
                title: Some(input.chars().take(80).collect()),
                agent,
                input,
                project_id: task_project_id,
                workspace_root,
                session_id: Some(task_session_id.clone()),
            })
            .await?;
        anyhow::Ok(result)
    });
    active_runs
        .insert(session_id.clone(), task.abort_handle())
        .await;
    let result = task.await;
    active_runs.remove(&session_id).await;
    match result {
        Ok(Ok(result)) => Ok(json!({
            "session_id": result.session_id,
            "project_id": project_id,
            "final": result.final_answer
        })),
        Ok(Err(err)) => Err(err),
        Err(join_error) if join_error.is_cancelled() => {
            Ok(json!({"session_id": session_id, "status": "cancelled", "final": ""}))
        }
        Err(join_error) => Err(anyhow::anyhow!("agent run task failed: {join_error}")),
    }
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

// --- Collaboration Engine API Routes ---

async fn api_projects_list(
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let projects = state.store.list_projects().await?;
    Ok(json_ok(json!({"projects": projects})))
}

async fn api_projects_create(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing project name"))?;
    let mode_str = body
        .get("collaboration_mode")
        .and_then(Value::as_str)
        .unwrap_or("automatic");
    let mode = match mode_str {
        "automatic" => CollaborationMode::Automatic,
        "structured" => CollaborationMode::Structured,
        "fast" => CollaborationMode::Fast,
        "careful" => CollaborationMode::Careful,
        "enterprise" => CollaborationMode::Enterprise,
        "airgapped" => CollaborationMode::Airgapped,
        _ => CollaborationMode::Automatic,
    };

    let project = state
        .collab_engine
        .create_project(name.to_string(), mode)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(json!(project)))
}

async fn api_projects_get(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let project = state.store.get_project(&id).await?;
    Ok(json_ok(json!(project)))
}

async fn api_projects_update(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    if let Some(mode_str) = body.get("collaboration_mode").and_then(Value::as_str) {
        let mode = match mode_str {
            "automatic" => CollaborationMode::Automatic,
            "structured" => CollaborationMode::Structured,
            "fast" => CollaborationMode::Fast,
            "careful" => CollaborationMode::Careful,
            "enterprise" => CollaborationMode::Enterprise,
            "airgapped" => CollaborationMode::Airgapped,
            _ => CollaborationMode::Automatic,
        };
        state
            .collab_engine
            .update_project_mode(&id, mode)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(json_ok(json!({"ok": true})))
}

async fn api_projects_delete(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    state.store.delete_project(&id).await?;
    Ok(json_ok(json!({"ok": true})))
}

async fn api_projects_add_agent(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let agent_id = body
        .get("agent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing agent_id"))?;
    let _ = state.store.get_agent(agent_id).await?;
    let project = state
        .collab_engine
        .add_agent_to_project(&id, agent_id)
        .await?;
    Ok(json_ok(json!({"project": project})))
}

async fn api_projects_remove_agent(
    Path((id, agent_id)): Path<(String, String)>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let project = state
        .collab_engine
        .remove_agent_from_project(&id, &agent_id)
        .await?;
    Ok(json_ok(json!({"project": project})))
}

async fn api_projects_invite(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let project = state.store.get_project(&id).await?;
    if project.collaboration_mode == CollaborationMode::Airgapped {
        return Err(ApiError::bad_request(
            "Airgapped policy blocks external invites",
        ));
    }
    let created_by = body
        .get("created_by")
        .and_then(Value::as_str)
        .unwrap_or("user");
    let mut permissions = parse_external_permissions(body.get("permissions"))?;
    if project.collaboration_mode == CollaborationMode::Enterprise {
        permissions.can_write_blackboard = false;
        permissions.can_create_artifacts = false;
        permissions.can_delegate_tasks = false;
        permissions.max_concurrent_tasks = permissions.max_concurrent_tasks.min(1);
    }
    let token = state
        .gateway
        .generate_invite(&id, permissions, created_by.to_string())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(json!({
        "token": token.token,
        "invite": token,
        "project_id": id
    })))
}

async fn api_projects_invites(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let _ = state.store.get_project(&id).await?;
    let invites = state.store.list_invite_tokens(&id).await?;
    Ok(json_ok(json!({"invites": invites})))
}

async fn api_projects_invite_revoke(
    Path((id, token)): Path<(String, String)>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let _ = state.store.get_project(&id).await?;
    state.store.revoke_invite_token(&id, &token).await?;
    Ok(json_ok(json!({"ok": true})))
}

fn parse_external_permissions(value: Option<&Value>) -> Result<ExternalPermissions, ApiError> {
    let Some(value) = value else {
        return Ok(ExternalPermissions::default());
    };
    let object = value
        .as_object()
        .ok_or_else(|| ApiError::bad_request("permissions must be an object"))?;
    let allowed_tools = object
        .get("allowed_tools")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let max_concurrent_tasks = object
        .get("max_concurrent_tasks")
        .and_then(Value::as_u64)
        .unwrap_or(2)
        .clamp(1, 16) as u32;
    Ok(ExternalPermissions {
        can_read_blackboard: object
            .get("can_read_blackboard")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        can_write_blackboard: object
            .get("can_write_blackboard")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        can_create_artifacts: object
            .get("can_create_artifacts")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        allowed_tools,
        can_delegate_tasks: object
            .get("can_delegate_tasks")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        max_concurrent_tasks,
    })
}

async fn api_projects_externals(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let externals = state.store.list_external_agents(&id).await?;
    Ok(json_ok(json!({"externals": externals})))
}

async fn api_projects_externals_remove(
    Path((_id, ext_id)): Path<(String, String)>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    state.store.delete_external_agent(&ext_id).await?;
    Ok(json_ok(json!({"ok": true})))
}

async fn api_projects_blackboard(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let entries = state.store.get_blackboard(&id).await?;
    Ok(json_ok(json!({"entries": entries})))
}

async fn api_projects_blackboard_write(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let key = body
        .get("key")
        .and_then(Value::as_str)
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("missing key"))?;
    let value = body
        .get("value")
        .cloned()
        .ok_or_else(|| ApiError::bad_request("missing value"))?;
    let author_id = body
        .get("author_id")
        .and_then(Value::as_str)
        .unwrap_or("user");
    let author_type = match body
        .get("author_type")
        .and_then(Value::as_str)
        .unwrap_or("system")
    {
        "local" => AuthorType::Local,
        "external" => AuthorType::External,
        "system" | "user" => AuthorType::System,
        other => {
            return Err(ApiError::bad_request(format!(
                "unsupported author_type `{other}`"
            )));
        }
    };
    let entry = state
        .collab_engine
        .blackboard_write(
            &id,
            key.to_string(),
            value,
            author_id.to_string(),
            author_type,
        )
        .await?;
    Ok(json_ok(json!({"entry": entry})))
}

async fn api_projects_tasks(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let tasks = state.store.list_collab_tasks(&id).await?;
    Ok(json_ok(json!({"tasks": tasks})))
}

async fn api_projects_tasks_submit(
    Path(id): Path<String>,
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response<Body>, ApiError> {
    require_auth(&headers, &state)?;
    let description = body
        .get("description")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing description"))?;

    let task = state
        .collab_engine
        .submit_task(&id, description.to_string())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(json_ok(json!({"task": task})))
}

async fn ws_gateway_connect(
    ws: WebSocketUpgrade,
    State(state): State<HttpState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, ApiError> {
    let token = params
        .get("token")
        .ok_or_else(|| ApiError::unauthorized("missing invite token"))?
        .clone();
    let agent_name = params
        .get("agent_name")
        .or_else(|| params.get("name"))
        .cloned()
        .unwrap_or_else(|| "external-agent".to_string());
    let gateway = state.gateway.clone();
    Ok(ws.on_upgrade(move |socket| async move {
        let mut socket = socket;
        let connection = match gateway.authenticate(&token, agent_name).await {
            Ok(connection) => connection,
            Err(err) => {
                let _ = socket
                    .send(Message::Text(
                        json!({"type":"error","message": err.to_string()})
                            .to_string()
                            .into(),
                    ))
                    .await;
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
        };
        let _ = socket
            .send(Message::Text(
                json!({
                    "type": "connected",
                    "external_agent_id": connection.external_agent_id,
                    "project_id": connection.project_id,
                    "permissions": connection.permissions,
                })
                .to_string()
                .into(),
            ))
            .await;
        while let Some(msg) = socket.recv().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response = match serde_json::from_str::<GatewayMessage>(&text) {
                        Ok(message) => gateway
                            .handle_message(&connection.external_agent_id, message)
                            .await
                            .map(|response| serde_json::to_value(response).unwrap_or_else(|_| {
                                json!({"type":"error","message":"failed to serialize response"})
                            }))
                            .unwrap_or_else(|err| {
                                json!({"type":"error","message": err.to_string()})
                            }),
                        Err(err) => json!({"type":"error","message": format!("invalid gateway message: {err}")}),
                    };
                    if socket
                        .send(Message::Text(response.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
        let _ = gateway.disconnect(&connection.external_agent_id).await;
    }))
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
    fn loop_completion_signal_requires_exact_text() {
        assert!(completion_seen(
            "AGENT1_LOOP_COMPLETE",
            &["done\nAGENT1_LOOP_COMPLETE"]
        ));
        assert!(!completion_seen(
            "AGENT1_LOOP_COMPLETE",
            &["done without the signal"]
        ));
        assert!(!completion_seen("", &["anything"]));
    }

    #[test]
    fn unknown_tool_fails_with_useful_error() {
        let text = valid_agent_toml("").replace("tools = [\"file_read\"]", "tools = [\"nope\"]");
        let agent = toml::from_str::<Agent>(&text).expect("agent parses before validation");
        let error = validate_agent_tools(&agent).expect_err("unknown tool should fail");
        assert!(error.to_string().contains("unknown tool `nope`"));
    }

    #[tokio::test]
    async fn cancel_session_aborts_registered_run() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let store = SqliteStore::connect(temp.path().join("test.db"))
            .await
            .expect("store");
        let session = store
            .create_session_shell("agent1", Some("cancel test".to_string()), None)
            .await
            .expect("session");
        let active_runs = Arc::new(ActiveRunRegistry::default());
        let task = tokio::spawn(async {
            sleep(Duration::from_secs(30)).await;
        });
        active_runs
            .insert(session.id.clone(), task.abort_handle())
            .await;

        let response = cancel_session_with_abort(&session.id, &store, active_runs)
            .await
            .expect("cancel");
        assert_eq!(response["aborted"], true);
        assert!(task
            .await
            .expect_err("task should be aborted")
            .is_cancelled());

        let saved = store.get_session(&session.id).await.expect("saved session");
        assert_eq!(saved.status, SessionStatus::Cancelled);
    }
}
