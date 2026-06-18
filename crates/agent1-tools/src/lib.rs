use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use agent1_core::{Agent1Error, Result, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{fs, process::Command, time};

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub workspace_root: PathBuf,
    pub agent_id: String,
    pub session_id: String,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult>;
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.insert(FileReadTool);
        registry.insert(FileListTool);
        registry.insert(FileWriteTool);
        registry.insert(GitStatusTool);
        registry.insert(GitDiffTool);
        registry.insert(WorkspaceSearchTool);
        registry.insert(TaskBoardTool);
        registry.insert(VerificationCheckTool);
        registry.insert(ShellTool);
        registry
    }

    pub fn insert<T: Tool + 'static>(&mut self, tool: T) {
        self.tools
            .insert(tool.definition().name.clone(), Arc::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn definitions_for(&self, names: &[String]) -> Vec<ToolDefinition> {
        names
            .iter()
            .filter_map(|name| self.tools.get(name).map(|tool| tool.definition()))
            .collect()
    }
}

fn resolve_existing_workspace_path(root: &Path, requested: &str) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|err| {
        Agent1Error::Runtime(format!(
            "workspace `{}` is not accessible: {err}",
            root.display()
        ))
    })?;
    let target = root.join(requested);
    let target = target.canonicalize().map_err(|err| {
        Agent1Error::Runtime(format!("path `{requested}` is not accessible: {err}"))
    })?;
    if !target.starts_with(&root) {
        return Err(Agent1Error::PathEscapesWorkspace(requested.to_string()));
    }
    Ok(target)
}

fn resolve_write_workspace_path(root: &Path, requested: &str) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|err| {
        Agent1Error::Runtime(format!(
            "workspace `{}` is not accessible: {err}",
            root.display()
        ))
    })?;
    let target = root.join(requested);
    let parent = target
        .parent()
        .ok_or_else(|| Agent1Error::Config(format!("path `{requested}` has no parent")))?;
    let parent = parent.canonicalize().map_err(|err| {
        Agent1Error::Runtime(format!("parent for `{requested}` is not accessible: {err}"))
    })?;
    if !parent.starts_with(&root) {
        return Err(Agent1Error::PathEscapesWorkspace(requested.to_string()));
    }
    Ok(target)
}

fn resolve_workspace_directory(root: &Path, requested: Option<&str>) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|err| {
        Agent1Error::Runtime(format!(
            "workspace `{}` is not accessible: {err}",
            root.display()
        ))
    })?;
    let target = match requested {
        Some(path) if !path.trim().is_empty() => root.join(path),
        _ => root.clone(),
    };
    let target = target.canonicalize().map_err(|err| {
        Agent1Error::Runtime(format!(
            "directory `{}` is not accessible: {err}",
            target.display()
        ))
    })?;
    if !target.starts_with(&root) {
        return Err(Agent1Error::PathEscapesWorkspace(
            requested.unwrap_or(".").to_string(),
        ));
    }
    if !target.is_dir() {
        return Err(Agent1Error::Config(format!(
            "`{}` is not a directory",
            target.display()
        )));
    }
    Ok(target)
}

#[derive(Debug, Deserialize)]
struct FileReadInput {
    path: String,
    #[serde(default)]
    max_bytes: Option<usize>,
}

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_read".to_string(),
            description: "Read a UTF-8 file inside the configured workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {"type": "string"},
                    "max_bytes": {"type": "integer", "minimum": 1}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: FileReadInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid file_read input: {err}")))?;
        let path = resolve_existing_workspace_path(&ctx.workspace_root, &input.path)?;
        let bytes = fs::read(&path).await.map_err(|err| {
            Agent1Error::Runtime(format!("failed to read `{}`: {err}", path.display()))
        })?;
        let max_bytes = input.max_bytes.unwrap_or(24_000);
        let truncated = bytes.len() > max_bytes;
        let bytes = if truncated {
            &bytes[..max_bytes]
        } else {
            &bytes
        };
        let content = String::from_utf8_lossy(bytes).to_string();
        Ok(ToolResult {
            content,
            metadata: json!({
                "path": input.path,
                "bytes_read": bytes.len(),
                "truncated": truncated,
                "agent_id": ctx.agent_id,
                "session_id": ctx.session_id
            }),
        })
    }
}

#[derive(Debug, Deserialize)]
struct FileListInput {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    max_entries: Option<usize>,
}

pub struct FileListTool;

#[async_trait]
impl Tool for FileListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_list".to_string(),
            description: "List files and directories inside the configured workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "max_entries": {"type": "integer", "minimum": 1, "maximum": 500}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: FileListInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid file_list input: {err}")))?;
        let dir = resolve_workspace_directory(&ctx.workspace_root, input.path.as_deref())?;
        let workspace_root = ctx.workspace_root.canonicalize().map_err(|err| {
            Agent1Error::Runtime(format!(
                "workspace `{}` is not accessible: {err}",
                ctx.workspace_root.display()
            ))
        })?;
        let mut entries = fs::read_dir(&dir).await.map_err(|err| {
            Agent1Error::Runtime(format!("failed to list `{}`: {err}", dir.display()))
        })?;
        let max_entries = input.max_entries.unwrap_or(120).min(500);
        let mut rows = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to read directory entry: {err}")))?
        {
            if rows.len() >= max_entries {
                break;
            }
            let metadata = entry
                .metadata()
                .await
                .map_err(|err| Agent1Error::Runtime(format!("failed to read metadata: {err}")))?;
            let path = entry.path();
            let relative = path.strip_prefix(&workspace_root).unwrap_or(&path);
            rows.push(json!({
                "path": relative.to_string_lossy().replace('\\', "/"),
                "kind": if metadata.is_dir() { "directory" } else { "file" },
                "bytes": if metadata.is_file() { Some(metadata.len()) } else { None },
            }));
        }
        rows.sort_by(|left, right| {
            left["path"]
                .as_str()
                .unwrap_or_default()
                .cmp(right["path"].as_str().unwrap_or_default())
        });
        Ok(ToolResult {
            content: serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".to_string()),
            metadata: json!({"entries": rows.len(), "truncated": rows.len() >= max_entries}),
        })
    }
}

#[derive(Debug, Deserialize)]
struct FileWriteInput {
    path: String,
    content: String,
}

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_write".to_string(),
            description: "Write a UTF-8 file inside the configured workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: FileWriteInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid file_write input: {err}")))?;
        let path = resolve_write_workspace_path(&ctx.workspace_root, &input.path)?;
        fs::write(&path, input.content.as_bytes())
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!("failed to write `{}`: {err}", path.display()))
            })?;
        Ok(ToolResult {
            content: format!("wrote {} bytes to {}", input.content.len(), input.path),
            metadata: json!({"path": input.path, "bytes_written": input.content.len()}),
        })
    }
}

pub struct GitStatusTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_status".to_string(),
            description: "Run `git status --short` in the workspace.".to_string(),
            input_schema: json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, _input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&ctx.workspace_root)
            .arg("status")
            .arg("--short")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to run git status: {err}")))?;
        let mut content = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            content.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        Ok(ToolResult {
            content,
            metadata: json!({"exit_code": output.status.code()}),
        })
    }
}

#[derive(Debug, Deserialize)]
struct GitDiffInput {
    #[serde(default)]
    staged: bool,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    max_bytes: Option<usize>,
}

pub struct GitDiffTool;

#[async_trait]
impl Tool for GitDiffTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_diff".to_string(),
            description:
                "Run `git diff` in the workspace, optionally for staged changes or a path."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "staged": {"type": "boolean"},
                    "path": {"type": "string"},
                    "max_bytes": {"type": "integer", "minimum": 1}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: GitDiffInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid git_diff input: {err}")))?;
        if let Some(path) = input.path.as_deref() {
            let _ = resolve_existing_workspace_path(&ctx.workspace_root, path)?;
        }
        let mut command = Command::new("git");
        command.arg("-C").arg(&ctx.workspace_root).arg("diff");
        if input.staged {
            command.arg("--staged");
        }
        if let Some(path) = input.path.as_deref() {
            command.arg("--").arg(path);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = command
            .output()
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to run git diff: {err}")))?;
        let mut content = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            content.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        let max_bytes = input.max_bytes.unwrap_or(48_000);
        let truncated = content.len() > max_bytes;
        if truncated {
            content.truncate(max_bytes);
            content.push_str("\n[truncated]");
        }
        Ok(ToolResult {
            content,
            metadata: json!({"exit_code": output.status.code(), "staged": input.staged, "truncated": truncated}),
        })
    }
}

#[derive(Debug, Deserialize)]
struct WorkspaceSearchInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

pub struct WorkspaceSearchTool;

#[async_trait]
impl Tool for WorkspaceSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "workspace_search".to_string(),
            description: "Search text in workspace files using ripgrep when available.".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "glob": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 500}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: WorkspaceSearchInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid workspace_search input: {err}")))?;
        if input.pattern.trim().is_empty() {
            return Err(Agent1Error::Config(
                "workspace_search pattern cannot be empty".to_string(),
            ));
        }
        let search_dir = resolve_workspace_directory(&ctx.workspace_root, input.path.as_deref())?;
        let mut command = Command::new("rg");
        command
            .arg("--line-number")
            .arg("--column")
            .arg("--no-heading")
            .arg("--color")
            .arg("never");
        if let Some(glob) = input.glob.as_deref() {
            command.arg("--glob").arg(glob);
        }
        command.arg(&input.pattern).arg(&search_dir);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = command
            .output()
            .await
            .map_err(|err| Agent1Error::Runtime(format!("failed to run ripgrep: {err}")))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let max_results = input.max_results.unwrap_or(80).min(500);
        let mut lines = stdout
            .lines()
            .take(max_results)
            .collect::<Vec<_>>()
            .join("\n");
        let total_seen = stdout.lines().count();
        let truncated = total_seen > max_results;
        if truncated {
            lines.push_str("\n[truncated]");
        }
        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                lines.push_str("\n[stderr]\n");
                lines.push_str(&stderr);
            }
        }
        Ok(ToolResult {
            content: lines,
            metadata: json!({
                "exit_code": output.status.code(),
                "matches_returned": total_seen.min(max_results),
                "truncated": truncated
            }),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskAction {
    Add,
    List,
    Update,
    Complete,
    Clear,
}

#[derive(Debug, Deserialize)]
struct TaskBoardInput {
    action: TaskAction,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskItem {
    id: String,
    title: String,
    status: String,
    #[serde(default)]
    notes: Option<String>,
}

pub struct TaskBoardTool;

#[async_trait]
impl Tool for TaskBoardTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "task_board".to_string(),
            description: "Maintain a small workspace-local task board in .agent1/tasks.json."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["action"],
                "properties": {
                    "action": {"type": "string", "enum": ["add", "list", "update", "complete", "clear"]},
                    "id": {"type": "string"},
                    "title": {"type": "string"},
                    "status": {"type": "string"},
                    "notes": {"type": "string"}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: TaskBoardInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid task_board input: {err}")))?;
        let path = resolve_write_workspace_path(&ctx.workspace_root, ".agent1/tasks.json")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                Agent1Error::Runtime(format!("failed to create task directory: {err}"))
            })?;
        }
        let mut tasks = read_tasks(&path).await?;
        match input.action {
            TaskAction::Add => {
                let title = input
                    .title
                    .filter(|title| !title.trim().is_empty())
                    .ok_or_else(|| {
                        Agent1Error::Config("task_board add requires title".to_string())
                    })?;
                let id = format!("task_{}", tasks.len() + 1);
                tasks.push(TaskItem {
                    id,
                    title,
                    status: input.status.unwrap_or_else(|| "todo".to_string()),
                    notes: input.notes,
                });
            }
            TaskAction::List => {}
            TaskAction::Update | TaskAction::Complete => {
                let id = input.id.ok_or_else(|| {
                    Agent1Error::Config("task_board update requires id".to_string())
                })?;
                let task = tasks
                    .iter_mut()
                    .find(|task| task.id == id)
                    .ok_or_else(|| Agent1Error::Config(format!("task `{id}` was not found")))?;
                if let Some(title) = input.title {
                    task.title = title;
                }
                if let Some(notes) = input.notes {
                    task.notes = Some(notes);
                }
                task.status = match input.action {
                    TaskAction::Complete => "done".to_string(),
                    _ => input.status.unwrap_or_else(|| task.status.clone()),
                };
            }
            TaskAction::Clear => {
                tasks.clear();
            }
        }
        fs::write(
            &path,
            serde_json::to_vec_pretty(&tasks)
                .map_err(|err| Agent1Error::Runtime(format!("failed to serialize tasks: {err}")))?,
        )
        .await
        .map_err(|err| Agent1Error::Runtime(format!("failed to write task board: {err}")))?;
        Ok(ToolResult {
            content: serde_json::to_string_pretty(&tasks).unwrap_or_else(|_| "[]".to_string()),
            metadata: json!({"path": ".agent1/tasks.json", "count": tasks.len()}),
        })
    }
}

async fn read_tasks(path: &Path) -> Result<Vec<TaskItem>> {
    match fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|err| Agent1Error::Runtime(format!("failed to parse task board: {err}"))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(Agent1Error::Runtime(format!(
            "failed to read task board `{}`: {err}",
            path.display()
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct VerificationCheckInput {
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    include_diff: bool,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

pub struct VerificationCheckTool;

#[async_trait]
impl Tool for VerificationCheckTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "verification_check".to_string(),
            description:
                "Run an allowlisted verification bundle in the workspace and summarize git state."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "commands": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": [
                                "cargo check",
                                "cargo test",
                                "cargo build",
                                "npm test",
                                "npm run check",
                                "npm run build"
                            ]
                        }
                    },
                    "include_diff": {"type": "boolean"},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 300}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: VerificationCheckInput = serde_json::from_value(input).map_err(|err| {
            Agent1Error::Config(format!("invalid verification_check input: {err}"))
        })?;
        let timeout_seconds = input.timeout_seconds.unwrap_or(120).min(300);
        let mut reports = Vec::new();
        reports
            .push(run_verification_command(&ctx.workspace_root, "git status --short", 10).await?);
        if input.include_diff {
            reports
                .push(run_verification_command(&ctx.workspace_root, "git diff --stat", 10).await?);
        }
        for command in input.commands {
            if !is_allowlisted_verification_command(&command) {
                return Err(Agent1Error::PermissionDenied(format!(
                    "`{command}` is not an allowlisted verification command"
                )));
            }
            reports.push(
                run_verification_command(&ctx.workspace_root, &command, timeout_seconds).await?,
            );
        }

        let passed = reports.iter().all(|report| {
            report
                .get("exit_code")
                .and_then(Value::as_i64)
                .map(|code| code == 0)
                .unwrap_or(false)
        });
        Ok(ToolResult {
            content: serde_json::to_string_pretty(&reports).unwrap_or_else(|_| "[]".to_string()),
            metadata: json!({
                "passed": passed,
                "checks": reports.len(),
                "timeout_seconds": timeout_seconds
            }),
        })
    }
}

fn is_allowlisted_verification_command(command: &str) -> bool {
    matches!(
        command,
        "cargo check"
            | "cargo test"
            | "cargo build"
            | "npm test"
            | "npm run check"
            | "npm run build"
    )
}

async fn run_verification_command(
    root: &Path,
    command: &str,
    timeout_seconds: u64,
) -> Result<Value> {
    let mut process = shell_command(command);
    process
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    process.kill_on_drop(true);
    let child = process
        .spawn()
        .map_err(|err| Agent1Error::Runtime(format!("failed to run `{command}`: {err}")))?;
    let output = time::timeout(
        Duration::from_secs(timeout_seconds),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| Agent1Error::Runtime(format!("`{command}` timed out after {timeout_seconds}s")))?
    .map_err(|err| Agent1Error::Runtime(format!("failed to run `{command}`: {err}")))?;
    let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
    truncate_field(&mut stdout, 12_000);
    truncate_field(&mut stderr, 12_000);
    Ok(json!({
        "command": command,
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr
    }))
}

fn truncate_field(text: &mut String, max_bytes: usize) {
    if text.len() > max_bytes {
        text.truncate(max_bytes);
        text.push_str("\n[truncated]");
    }
}

#[derive(Debug, Deserialize)]
struct ShellInput {
    command: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".to_string(),
            description: "Run a shell command in the workspace after explicit approval."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {"type": "string"},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 120}
                }
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolResult> {
        let input: ShellInput = serde_json::from_value(input)
            .map_err(|err| Agent1Error::Config(format!("invalid shell input: {err}")))?;
        validate_shell_command(&input.command)?;
        let timeout_seconds = input.timeout_seconds.unwrap_or(30).min(120);
        let mut command = shell_command(&input.command);
        command
            .current_dir(&ctx.workspace_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.kill_on_drop(true);
        let child = command
            .spawn()
            .map_err(|err| Agent1Error::Runtime(format!("failed to run shell command: {err}")))?;
        let output = time::timeout(
            Duration::from_secs(timeout_seconds),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| {
            Agent1Error::Runtime(format!("shell command timed out after {timeout_seconds}s"))
        })?
        .map_err(|err| Agent1Error::Runtime(format!("failed to run shell command: {err}")))?;
        let mut content = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            content.push_str("\n[stderr]\n");
            content.push_str(&stderr);
        }
        if content.len() > 24_000 {
            content.truncate(24_000);
            content.push_str("\n[truncated]");
        }
        Ok(ToolResult {
            content,
            metadata: json!({"exit_code": output.status.code(), "timeout_seconds": timeout_seconds}),
        })
    }
}

fn validate_shell_command(command: &str) -> Result<()> {
    let normalized = command
        .to_ascii_lowercase()
        .replace('`', "")
        .replace('\n', " ");
    let forbidden = [
        "rm -rf",
        "remove-item",
        " rmdir ",
        " rmdir/",
        " del /",
        "format ",
        "diskpart",
        "shutdown",
        "restart-computer",
        "stop-computer",
        "reg delete",
        "git reset --hard",
        "git clean -fd",
        ":(){",
    ];
    if forbidden.iter().any(|pattern| normalized.contains(pattern)) {
        return Err(Agent1Error::PermissionDenied(
            "shell command matches destructive denylist".to_string(),
        ));
    }
    if normalized.contains("remove-item") && normalized.contains("-recurse") {
        return Err(Agent1Error::PermissionDenied(
            "recursive removal is blocked by shell denylist".to_string(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile").arg("-Command").arg(command);
    cmd
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-lc").arg(command);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_default_tools() {
        let registry = ToolRegistry::with_defaults();
        assert!(registry.get("file_read").is_some());
        assert!(registry.get("file_list").is_some());
        assert!(registry.get("workspace_search").is_some());
        assert!(registry.get("git_diff").is_some());
        assert!(registry.get("task_board").is_some());
        assert!(registry.get("verification_check").is_some());
        assert!(registry.get("shell").is_some());
    }

    #[test]
    fn shell_denylist_blocks_destructive_commands() {
        assert!(validate_shell_command("Remove-Item -Recurse target").is_err());
        assert!(validate_shell_command("git reset --hard").is_err());
        assert!(validate_shell_command("cargo check").is_ok());
    }

    #[test]
    fn verification_command_allowlist_is_narrow() {
        assert!(is_allowlisted_verification_command("cargo check"));
        assert!(is_allowlisted_verification_command("npm run build"));
        assert!(!is_allowlisted_verification_command(
            "cargo test && git reset --hard"
        ));
        assert!(!is_allowlisted_verification_command(
            "powershell Remove-Item file"
        ));
    }

    #[tokio::test]
    async fn shell_timeout_is_handled() {
        let command = if cfg!(windows) {
            "Start-Sleep -Seconds 2"
        } else {
            "sleep 2"
        };
        let result = ShellTool
            .execute(
                json!({"command": command, "timeout_seconds": 1}),
                ToolContext {
                    workspace_root: std::env::current_dir().expect("cwd"),
                    agent_id: "agent".to_string(),
                    session_id: "session".to_string(),
                },
            )
            .await;
        assert!(result
            .expect_err("timeout expected")
            .to_string()
            .contains("timed out"));
    }
}
