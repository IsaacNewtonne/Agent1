# Agent1 Workspace Index

Use this file as the first navigation map before broad file listing or text search.

## Starting Points

- `README.md`: current product state, run commands, architecture, API highlights, and troubleshooting.
- `DESIGN.md`: visual source of truth for frontend work.
- `Agent1_Project_Docs/SPEC.md`: desktop UI product and design specification.
- `Agent1_Project_Docs/SELF_IMPROVE.md`: operating manual for self-improvement loops.
- `Agent1_Project_Docs/META_EVAL.md`: evaluation and meta-review guidance.
- `Agent1_Project_Docs/EXTERNAL_INTELLIGENCE_LOOP.md`: external intelligence loop design.
- `plans/`: roadmap and milestone plans.

## Core Rust Crates

- `crates/agent1-core`: shared types, permissions, errors, IDs, sessions, tools, models, projects, and events.
- `crates/agent1-tools`: native workspace tools such as file read/list/write, workspace map, search, git, task board, verification, and shell.
- `crates/agent1-runtime`: agent execution loop, model calls, tool approval flow, memory context injection, MCP calls, and runtime tool definitions.
- `crates/agent1-orchestrator`: goal decomposition, role agent creation, plan execution, critique, memory recall, and suggestions.
- `crates/agent1-cli`: CLI commands, local API server, HTTP routes, WebSocket routes, project endpoints, memory endpoints, and app bootstrap.
- `crates/agent1-db`: SQLite persistence and migrations for agents, sessions, projects, events, approvals, memory, collaboration, and suggestions.
- `crates/agent1-models`: model provider adapters for Ollama, OpenCode, NVIDIA, OpenAI-compatible endpoints, and mock test providers.
- `crates/agent1-memory`: semantic memory storage, embeddings, search, consolidation, and proactive suggestions.
- `crates/agent1-collab`: project collaboration engine, blackboard, collaboration tasks, and project mode behavior.
- `crates/agent1-gateway`: invite-token gateway for external Hermes-style agents.
- `crates/agent1-whatsapp`: WhatsApp sidecar integration.

## Desktop App

- `desktop/src/App.jsx`: main React app shell.
- `desktop/src/components/CollabWorkspace.jsx`: primary Hybrid Collaboration Workspace UI.
- `desktop/src/components/MemoryPanel.jsx`: memory and suggestion panel.
- `desktop/src/components/CanvasGraph.jsx`, `ProjectSphere.jsx`, `SharedWorkspaceVisual.jsx`: central visual workspace components.
- `desktop/src/hooks/useCollaboration.js`: collaboration state and API integration.
- `desktop/src/styles.css` and `desktop/src/collab.css`: main UI styling.
- `desktop/e2e/`: Playwright and visual QA scripts.

## Agent Profiles

- `agents/assistant.toml`: principal Agent1 operating agent.
- `agents/planner.toml`: planning-focused agent.
- `agents/worker.toml`: implementation worker.
- `agents/critic.toml`: adversarial reviewer.
- `agents/code_reviewer.toml`: code review agent.

## Common Workflows

- Rust verification: `cargo fmt`, `cargo test`, `cargo check`.
- Desktop development: start the API server, then run `npm run tauri:dev` or `npm run dev` from `desktop/`.
- When changing frontend visuals, read `DESIGN.md` first.
- When changing agent execution, inspect `crates/agent1-runtime/src/lib.rs`, then related tool definitions in `crates/agent1-tools/src/lib.rs`.
- When changing orchestration behavior, inspect `crates/agent1-orchestrator/src/orchestrator.rs`, `goal_decomposer.rs`, and `team_manager.rs`.
- When changing API routes, inspect `crates/agent1-cli/src/main.rs`.
