# Agent1 - Local-First Personal Agent Runtime

A local-first personal agent runtime with MCP server support, streaming model providers, and a desktop UI.

## Installation

### From Source

```bash
cargo build --release
```

### From Binary

Download the latest release from GitHub Releases.

## Quick Start

### 1. Start the API Server

```bash
cargo run --bin agent1 -- server --bind 127.0.0.1:17371
```

### 2. Launch Desktop UI

```bash
cd desktop
npm run tauri:dev
```

Or run the built application after `npm run tauri:build`.

## CLI Commands

| Command | Description |
|---------|-------------|
| `agent1 run --agent <file> --task <task>` | Run an agent |
| `agent1 server` | Start the API server |
| `agent1 models` | List available models |
| `agent1 agent list` | List saved agents |
| `agent1 memory write` | Write a memory |
| `agent1 mcp list` | List MCP servers |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/agents` | GET | List agents |
| `/api/agents` | POST | Create agent |
| `/api/sessions` | GET | List sessions |
| `/api/sessions/run` | POST | Run agent |
| `/api/sessions/{id}/trace` | GET | Get session trace |
| `/api/sessions/{id}/cancel` | POST | Cancel session |
| `/api/events` | GET | Get events |
| `/api/approvals` | GET | Get approvals |
| `/api/models` | GET | List model providers |
| `/api/mcp/servers` | GET | List MCP servers |
| `/api/mcp/servers` | POST | Add MCP server |
| `/api/memory` | GET/POST | Memory operations |
| `/ws/events` | WS | WebSocket events |
| `/api/health` | GET | Health check |

## Configuration

### Model Providers

- **ollama**: Local Ollama instance (default `http://localhost:11434`)
- **openai-compatible**: OpenAI-compatible APIs (default `http://localhost:8000/v1`)
- **mock**: Mock provider for testing

### Environment Variables

- `OLLAMA_BASE_URL`: Override Ollama base URL
- `OPENAI_BASE_URL`: Override OpenAI-compatible base URL

## Troubleshooting

### Ollama Connection Error

Ensure Ollama is running: `ollama serve`

### MCP Server Not Starting

Check that the command and arguments are valid. Use `agent1 mcp list` to verify.

### Port Already in Use

Use `--bind 127.0.0.1:17372` to use a different port.

## Security

- API server binds to loopback only by default
- Secrets in logs are automatically redacted
- File operations require explicit permissions
- MCP servers can be disabled individually