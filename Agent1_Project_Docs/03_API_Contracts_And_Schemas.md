# API Contracts and Schemas

## Local HTTP API

The local server is used by the desktop UI and optional local clients.

Base URL:

```text
http://127.0.0.1:17371
```

## Agents

### GET /api/agents

Returns all agents.

Response:

```json
{
  "agents": [
    {
      "id": "assistant",
      "name": "Assistant",
      "description": "General local assistant"
    }
  ]
}
```

### POST /api/agents

Creates an agent.

Request:

```json
{
  "id": "code-reviewer",
  "name": "Code Reviewer",
  "description": "Reviews Rust code",
  "system_prompt": "You are a senior Rust code reviewer.",
  "model": {
    "provider": "ollama",
    "model": "qwen3.5:4b",
    "base_url": "http://localhost:11434",
    "context_window": 65536,
    "temperature": 0.2
  },
  "tools": ["file_read", "git_status"],
  "permissions": {
    "file_read": "ask",
    "file_write": "ask",
    "shell": "ask",
    "network": "deny"
  }
}
```

## Sessions

### POST /api/sessions

Creates a session.

Request:

```json
{
  "root_agent_id": "assistant",
  "title": "New task"
}
```

Response:

```json
{
  "session_id": "sess_01"
}
```

### POST /api/sessions/{session_id}/run

Runs an agent task.

Request:

```json
{
  "agent_id": "assistant",
  "input": "Review this project.",
  "stream": true
}
```

Response:

```json
{
  "run_id": "run_01",
  "status": "running"
}
```

### POST /api/sessions/{session_id}/cancel

Cancels an active run.

Response:

```json
{
  "status": "cancelled"
}
```

## Tool Approval

### POST /api/tool-approvals/{approval_id}

Request:

```json
{
  "decision": "approved"
}
```

Allowed decisions:

```text
approved
denied
always_allow_for_session
```

## Models

### GET /api/models

Response:

```json
{
  "providers": [
    {
      "provider": "ollama",
      "models": ["qwen3.5:4b", "llama3.1:8b"]
    }
  ]
}
```

## Agent Card

### GET /.well-known/agent.json

Response:

```json
{
  "id": "assistant",
  "name": "Assistant",
  "description": "General local assistant",
  "skills": [
    {
      "name": "general_chat",
      "description": "Answer general questions"
    }
  ],
  "input_modes": ["text"],
  "output_modes": ["text", "markdown"],
  "endpoint": "http://127.0.0.1:17371/api/agents/assistant/tasks"
}
```

## Error Response

All API errors use:

```json
{
  "error": {
    "code": "permission_denied",
    "message": "The agent is not allowed to use shell commands.",
    "details": {}
  }
}
```
