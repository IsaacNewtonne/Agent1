# System Architecture Overview

## Architecture Summary

Agent1 is a modular Rust application with a local runtime, local data store, model adapters, tool adapters, MCP integration, A2A-style agent messaging, CLI, server, and Tauri UI.

## High-Level Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                       UI Layer                          │
│        Tauri Desktop / CLI / Local HTTP Server          │
└────────────────────────────┬────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────┐
│                    Agent1 Runtime                       │
│ Agent Runner / Prompt Builder / Tool Loop / Events      │
└──────────────┬─────────────┬──────────────┬─────────────┘
               │             │              │
┌──────────────▼───┐ ┌───────▼───────┐ ┌────▼────────────┐
│ Model Providers  │ │ Tool Registry │ │ Memory System   │
│ Ollama/local     │ │ Native + MCP  │ │ SQLite/search   │
└──────────────┬───┘ └───────┬───────┘ └────┬────────────┘
               │             │              │
┌──────────────▼─────────────▼──────────────▼─────────────┐
│                    Local Infrastructure                  │
│ Ollama / MCP Servers / SQLite / Filesystem / Git         │
└─────────────────────────────────────────────────────────┘
```

## Main Runtime Flow

```text
User task
↓
Create/load session
↓
Load host agent
↓
Build prompt
↓
Call local model
↓
Parse response
↓
If tool call: permission check → tool execution → result back to session
↓
If agent handoff: call target agent
↓
If final answer: store and return
```

## Core Crates

| Crate | Responsibility |
|---|---|
| `agent1-core` | Domain objects and schemas |
| `agent1-runtime` | Agent execution loop |
| `agent1-models` | Local model providers |
| `agent1-tools` | Native tool system |
| `agent1-memory` | Memory storage and retrieval |
| `agent1-mcp` | MCP client support |
| `agent1-a2a` | Agent cards and agent messaging |
| `agent1-db` | SQLite migrations and repositories |
| `agent1-server` | Local HTTP/WebSocket server |
| `agent1-cli` | Developer CLI |
| `agent1-common` | Shared utilities |

## Design Principles

- Runtime does not depend on UI.
- Tools do not directly call model providers.
- Model providers do not know about sessions.
- MCP tools are adapted into the same tool registry as native tools.
- Every important action emits an event.
- All user data is local unless explicitly configured otherwise.
