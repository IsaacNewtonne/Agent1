# Agent1

Agent1 is a local-first AI workspace for running a central orchestrator, local worker agents, and permissioned external collaborators from one desktop surface.

The current app centers on the **Hybrid Collaboration Workspace**: create a project, choose how Agent1 should coordinate the work, attach local or external agents, then talk to Agent1 from the command box while approvals and activity stream through the workspace.

## Current App State

- 🧭 **Hybrid Collaboration Workspace**: project-first desktop UI with a central Agent1 canvas, local systems lane, external agents lane, activity feed, and command composer.
- 🗂️ **Project workflow**: create and select projects, keep the active project remembered, and switch collaboration mode from the header.
- 🎛️ **Agent1 orchestration**: Agent1 is the central user-facing controller. Click the Agent1 node to choose provider and model from available dropdowns.
- 🧑‍💻 **Local agents**: add local worker agents, configure provider/model, and delete agents with a custom confirmation dialog.
- 🌐 **External agents and MCP servers**: invite or connect external systems from the right lane for project-scoped collaboration.
- 🛡️ **Permissions and approvals**: local tools use explicit permission policies, external agents use project-scoped permissions, and pending approvals appear in the activity area.
- 🧠 **Model providers**: supports OpenCode, Ollama, OpenAI-compatible endpoints, and NVIDIA NIM.
- ⚡ **Live runtime**: sessions, events, approvals, projects, and model options are served through the local API.

## Simple User Path

1. 🚦 **Start the API server**

   ```powershell
   cargo run --bin agent1 -- server --bind 127.0.0.1:17371
   ```

2. 🖥️ **Open the desktop app**

   ```powershell
   cd desktop
   npm install
   npm run tauri:dev
   ```

   For browser-only development:

   ```powershell
   cd desktop
   npm run dev
   ```

   Then open `http://localhost:1420`.

3. ✨ **Create a project**

   Use `+ Create Project`, enter a project name, and choose a collaboration mode:

   | Mode | Use When |
   |------|----------|
   | 🤖 `Automatic` | Let Agent1 choose the coordination style based on context. |
   | 📋 `Structured` | You want a plan, delegated execution, and review. |
   | 🚀 `Fast` | You want lower-overhead parallel work. |
   | 🛡️ `Careful` | You want more approval checkpoints before actions. |

4. 🔐 **Choose permissions**

   Use conservative permissions first:

   - 🧰 Give local agents only the tools they need, such as file read, file list, search, memory, or MCP access.
   - 🚧 Keep file write and shell-style capabilities denied unless the task requires them.
   - 🪪 For external agents, prefer project-scoped permissions: read blackboard, write blackboard, create artifacts, allowed tools, delegation, and max concurrent tasks.
   - ✅ Review approval prompts in the activity feed before allowing sensitive actions.

5. 🎚️ **Configure Agent1**

   Click the central `A1` node, choose a provider and model from the dropdowns, then save. The setup window closes after saving.

6. 🤝 **Add collaborators**

   - 🧑‍💻 Use `+ Add Agent` for local workers.
   - 🌍 Use `+ Invite External` for external agents or MCP-backed systems.
   - 🗑️ Delete local agents from their lane when they are no longer needed.

7. 💬 **Run work**

   Type into `Agent1 Command` and press `Run`. Agent1 handles the request, records the session, streams activity, and coordinates available agents according to the active project mode.

## Architecture

| Layer | Location | Purpose |
|-------|----------|---------|
| 🖼️ Desktop UI | `desktop/` | React/Tauri Hybrid Collaboration Workspace. |
| 🧩 CLI/API | `crates/agent1-cli/` | Local API server, CLI commands, routes, and app bootstrap. |
| ⚙️ Runtime | `crates/agent1-runtime/` | Agent session execution, tools, approvals, events, and failure handling. |
| 🧠 Models | `crates/agent1-models/` | OpenCode, Ollama, OpenAI-compatible, and NVIDIA NIM model adapters. |
| 🗄️ Database | `crates/agent1-db/` | SQLite persistence for agents, sessions, projects, events, approvals, and collaboration records. |
| 🧭 Collaboration | `crates/agent1-collab/` | Project state, collaboration modes, blackboard, tasks, and external agent records. |
| 🌉 Gateway | `crates/agent1-gateway/` | Invite-token flow and project-scoped external agent access. |

## API Highlights

| Endpoint | Description |
|----------|-------------|
| 🩺 `GET /api/health` | Check the local API server. |
| 🧑‍💻 `GET /api/agents` | List local agents, including Agent1. |
| ➕ `POST /api/agents` | Create or update a local agent. |
| 🗑️ `DELETE /api/agents/{agent_id}` | Delete a local agent. Agent1 cannot be deleted. |
| 🧠 `GET /api/models` | List available models by provider. |
| 🗂️ `GET /api/projects` | List collaboration projects. |
| ✨ `POST /api/projects` | Create a project with a collaboration mode. |
| 🎛️ `PATCH /api/projects/{id}` | Update project settings such as collaboration mode. |
| 🪪 `POST /api/projects/{id}/invite` | Generate an external invite token for a project. |
| 🌐 `GET /api/projects/{id}/externals` | List external agents for a project. |
| 🧾 `GET /api/projects/{id}/blackboard` | Read project blackboard entries. |
| ✅ `GET /api/projects/{id}/tasks` | List collaboration tasks. |
| 📬 `POST /api/projects/{id}/tasks` | Submit a collaboration task. |
| 💬 `POST /api/sessions/run` | Run Agent1 or another agent. |
| 🔎 `GET /api/sessions/{id}/trace` | Inspect messages, events, tool calls, and approvals for a session. |
| ⚡ `GET /ws/events` | Receive live runtime events. |

## CLI Commands

| Command | What It Does |
|---------|--------------|
| 🚦 `agent1 server` | Start the local API server. |
| 💬 `agent1 run --agent agents/assistant.toml --task "hello"` | Run an agent from the CLI. |
| 👥 `agent1 team --task "..."` | Run planner, worker, and critic agents. |
| 🔁 `agent1 loop --task "..." --max-runs 5` | Run bounded autonomous plan, execute, review iterations with persistent notes. |
| 🧠 `agent1 models --provider ollama` | List models for a provider. Use `--provider nvidia` for NVIDIA NIM. |
| ➕ `agent1 agent create agents/my-agent.toml` | Save an agent definition. |
| 🧰 `agent1 mcp list` | List MCP servers. |
| 📚 `agent1 sessions` | List recent sessions. |
| ⚡ `agent1 events` | List recent runtime events. |

## Model Configuration

### Model Routing

Orchestrated runs route roles independently. By default every role uses Ollama `llama3.1:8b`, but you can override globally or per role:

```powershell
$env:AGENT1_MODEL_DEFAULT = "llama3.1:8b"
$env:AGENT1_MODEL_CRITIC = "qwen2.5-coder:14b"
$env:AGENT1_MODEL_PLANNER_PROVIDER = "openai_compatible"
$env:AGENT1_MODEL_PLANNER_BASE_URL = "http://localhost:8000/v1"
```

Supported role suffixes are `PLANNER`, `WORKER`, `CRITIC`, `RESEARCHER`, `BUILDER`, `REPORTER`, and `ORCHESTRATOR`. Each role also supports `_PROVIDER`, `_BASE_URL`, and `_CONTEXT` overrides.

### OpenCode

🧠 OpenCode is available as a provider in the Agent1 configuration dropdown. On Windows, Agent1 launches the OpenCode Node entrypoint directly to avoid shell argument issues with `opencode.cmd`.

### Ollama

🦙 Run Ollama locally, then configure Agent1 or a worker agent with provider `ollama`.

```powershell
ollama serve
```

🔌 Optional base URL:

```powershell
$env:OLLAMA_BASE_URL = "http://localhost:11434"
```

### OpenAI-Compatible APIs

🌐 Use this for local or hosted OpenAI-compatible servers:

```powershell
$env:OPENAI_BASE_URL = "http://localhost:8000/v1"
```

### NVIDIA NIM

🚀 Agent1 supports NVIDIA NIM through the OpenAI-compatible chat completions and models APIs. Set your NVIDIA API key before starting the API server or desktop app:

```powershell
$env:NVIDIA_API_KEY = "nvapi-..."
cargo run --bin agent1 -- models --provider nvidia
```

The default NVIDIA base URL is `https://integrate.api.nvidia.com/v1`. Agent definitions should use provider `nvidia` and one of the model IDs returned by the model listing command or the desktop dropdown.

## Troubleshooting

### The desktop cannot connect

🩺 Confirm the API is running:

```powershell
Invoke-RestMethod http://127.0.0.1:17371/api/health
```

### The command box returns an error

🔎 Check Agent1's provider/model from the central `A1` configuration panel. Then inspect recent sessions and events:

```powershell
cargo run --bin agent1 -- sessions
cargo run --bin agent1 -- events
```

### The project does not appear after creating it

🔄 Refresh the desktop. Projects are persisted in SQLite and the UI remembers the active project.

### A run looks stuck

🧹 Restarting the API marks interrupted `running` sessions as `failed`, so stale sessions should not keep the Active counter stuck.

### Port in use

🚪 Start the API on another local port:

```powershell
cargo run --bin agent1 -- server --bind 127.0.0.1:17372
```

🎛️ Then update the desktop API base setting.

## Privacy and Safety

- 🏠 Agent1 binds the API to localhost for local-first use.
- 🗄️ Project, session, agent, and approval data are stored locally in SQLite.
- 🛡️ Tool use is permissioned; sensitive actions should remain approval-gated.
- 🪪 External agents are project-scoped and should be granted only the permissions needed for the current project.
