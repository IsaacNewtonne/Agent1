# 🤖 Agent1 - Your Local AI Assistant

A local-first personal AI agent that runs on your machine. No cloud, no account required.

## ✨ What It Does

- **Runs AI agents locally** - Your data stays on your device
- **Connects to Ollama** - Uses local AI models (Llama, Mistral, etc.)
- **MCP support** - Works with 80+ tools (filesystem, Git, Slack, etc.)
- **Desktop app** - Clean UI for managing agents
- **Persistent memory** - Remembers conversations

## 🚀 Quick Start

### Start the Server

```bash
cargo run --bin agent1 -- server --bind 127.0.0.1:17371
```

### Open the Desktop App

```bash
cd desktop
npm install
npm run tauri:dev
```

## 💻 CLI Commands

| Command | What It Does |
|---------|--------------|
| `agent1 run --agent my-agent.toml --task "hello"` | Run an agent |
| `agent1 server` | Start the API server |
| `agent1 models` | See available AI models |
| `agent1 memory write "note"` | Save a note |

## 🌐 API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/agents` | List your agents |
| `POST /api/sessions/run` | Run a task |
| `GET /api/sessions/{id}/stream` | Stream progress |
| `GET /ws/events` | Real-time events |

## ⚙️ Configuration

### Connect to Ollama

```bash
export OLLAMA_BASE_URL=http://localhost:11434
```

### Use OpenAI-Compatible APIs

```bash
export OPENAI_BASE_URL=http://localhost:8000/v1
```

## 🔧 Troubleshooting

### Can't connect to Ollama?
Run `ollama serve` in another terminal.

### Port in use?
Try: `cargo run --bin agent1 -- server --bind 127.0.0.1:17372`

### MCP server not working?
Check: `agent1 mcp list`

## 🔒 Privacy

- Runs locally - data never leaves your machine
- API binds to localhost only
- Secrets are automatically redacted in logs