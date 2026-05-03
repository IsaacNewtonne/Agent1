# Blueprint: Agent1 Milestones 5–10 Completion

## Context

Agent1 is a local-first personal agent runtime with:
- **Rust workspace**: `agent1-core`, `agent1-tools`, `agent1-models`, `agent1-db`, `agent1-runtime`, `agent1-cli`
- **Legacy UI**: `app/` — vanilla HTML/CSS/JS static mission-control shell served by the Axum server
- **New UI**: `desktop/` — React/Tauri scaffold, more feature-rich, not yet wired to all backend capabilities
- **Server**: Axum-based loopback HTTP+WebSocket API on port 17371
- **Models**: mock, Ollama (full streaming), OpenAI-compatible (non-streaming)
- **Storage**: SQLite via `agent1-db`
- **MCP**: stdio session pool via `OnceLock<Mutex<HashMap>>`, no health checks, no bounds

The project README says the React/Tauri desktop migration is "scaffolded in `desktop/`" — it builds and runs but is missing critical features and isn't yet the primary entry point.

---

## What Still Needs to Be Done

1. Retire `app/` as primary — redirect to `desktop` or serve it from the Tauri app
2. Desktop parity: MCP manager, memory viewer/editor, session cancel UX
3. Runtime hardening: MCP pool health checks, bounded session limits, graceful shutdown
4. Streaming parity: OpenAI-compatible actual streaming + SSE UI endpoint
5. Verification expansion: API/MCP/memory/security integration tests + desktop E2E + stable CI
6. Milestone 10 release: installer/distribution, docs/examples polish, security/release smoke checks

---

## Step Dependency Graph

```
Step 1  [app retirement]
Step 2  [mcp manager]       ←depends on nothing (parallel with 3,4,5)
Step 3  [memory viewer]      ←depends on nothing (parallel with 2,4,5)
Step 4  [session cancel]     ←depends on nothing (parallel with 2,3,5)
Step 5  [desktop other]      ←depends on nothing (parallel with 2,3,4)
Step 6  [runtime hardening]  ←depends on 1 (uses cancel API added in 4)
Step 7  [streaming parity]   ←depends on 1
Step 8  [integration tests]  ←depends on 1,6,7
Step 9  [e2e + security]     ←depends on 1,2,3,4,5
Step 10 [ci setup]           ←depends on 8,9
Step 11 [installer/release]  ←depends on 10
Step 12 [docs/smoke]         ←depends on 11
```

---

## Step 1: Retire `app/` — Make `desktop` Primary

### Context

- `app/index.html` + `app/app.js` + `app/styles.css` — vanilla HTML shell with demo trace, API polling, WS events, approval modal, agent builder
- `desktop/` — React + Vite + Tauri 2.x, superior UI but incomplete features
- `agent1-cli` `Command::Ui` prints `app/index.html` path; server serves `app/` static files
- Tauri desktop builds a standalone native binary

### Tasks

1. **Redirect `agent1 ui` command** to launch the Tauri app (`desktop/` built binary or `npm run tauri:dev`). Remove the `app/` path print.
2. **Update `run_server`** to drop static serving of `app/index.html`, `app/app.js`, `app/styles.css`, `Agent1_Project_Docs/Logo.png`. Keep all API routes.
3. **Add Tauri IPC bridge** so desktop shell can call backend API at `http://127.0.0.1:17371` (or embed the server in-process — see decision below).
4. **Wire up desktop's API calls** to use the Tauri fetch wrapper or in-process Rust calls via `tauri::command`.
5. **Delete or archive `app/`** — either remove from tree or move to `docs/legacy-app/` with a note.
6. **Update README** — `agent1 ui` → Tauri dev/run; add desktop build instructions to docs.
7. **Verify**: `cargo run -p agent1-cli -- ui` opens the desktop app; the old HTML UI no longer works.

> **In-process vs separate-server decision**: The simplest path for MVP is to keep the Axum server as a separate process (`agent1 server`) and have the Tauri app's JS call `http://127.0.0.1:17371` directly. A future iteration could embed the server in the Tauri binary via `tauri-plugin-shell` or a custom Rust command. Do NOT try to embed in this milestone — too much complexity.

### Exit Criteria

- `agent1 ui` launches the Tauri desktop app
- All existing API routes still work via desktop JS (`/api/agents`, `/api/sessions`, etc.)
- `app/` directory is removed or archived
- README reflects the new flow

---

## Step 2: Desktop Parity — MCP Manager Panel

### Context

- `desktop/src/App.jsx` already shows MCP servers in a read-only panel (`/api/mcp/servers`)
- No ability to add, remove, enable/disable, or call tools on MCP servers from the UI
- Backend has `McpCommand::Add`, `api_mcp_servers` (GET only), `McpCommand::Tools`, `McpCommand::Call`

### Tasks

1. **Add MCP add form** to the Control Surface rail — fields: `name`, `command`, `args` (comma-separated), `enabled` checkbox. POST to `POST /api/mcp/servers` (currently no create route — add it to `api_mcp_servers_create`).
2. **Add MCP servers create route** to the Axum server: `POST /api/mcp/servers` that calls `store.save_mcp_server()`.
3. **Add delete/disable controls** to each MCP server card in the desktop UI — `DELETE /api/mcp/servers/{id}` route, enable/disable `PATCH /api/mcp/servers/{id}`.
4. **Add MCP tools browser** — click a server card → slide-over or modal showing `GET /api/mcp/tools/{server}` (or `mcp/tools?server=name`).
5. **Wire refresh** to re-fetch `mcp_servers` after mutations.

### Exit Criteria

- Can add a new MCP server from the desktop UI
- Can delete/disable an MCP server from the desktop UI
- Can browse tools on an MCP server
- No console errors; API calls return correct status codes

---

## Step 3: Desktop Parity — Memory Viewer/Editor

### Context

- `desktop/App.jsx` has no memory tab — the memory view just shows events that start with `memory_`
- Backend has `MemoryCommand::Write`, `Search`, `Delete`; HTTP API has no memory endpoints yet
- `agent1-runtime` emits `MemoryRead` and `MemoryWritten` events

### Tasks

1. **Add memory HTTP endpoints** to `run_server`:
   - `GET /api/memory?agent={id}&query={q}&limit={n}` — search memories
   - `POST /api/memory` — write memory item
   - `DELETE /api/memory/{id}` — delete
2. **Add a Memory tab** to the Session Explorer in `desktop/App.jsx`:
   - Search bar with agent filter
   - List of memory items (scope, tags, content preview, importance)
   - "New memory" form (content, scope selector, tags, importance)
   - Click to expand/edit/delete
3. **Reuse `Store::search_memories`, `Store::write_memory`, `Store::delete_memory`**
4. **Wire memory events** from the event feed to also appear in the memory view

### Exit Criteria

- Memory search from desktop UI shows stored memories
- Can write a new memory from desktop UI
- Can delete a memory from desktop UI
- Backend endpoints return correct JSON shapes

---

## Step 4: Desktop Parity — Session Cancel UX

### Context

- `api_session_cancel` already exists in the Axum server — it sets session status to `Cancelled` and emits `RunCancelled` event
- `AgentRuntime::run` checks `SessionStatus::Cancelled` at the start of each iteration and returns early
- Desktop `App.jsx` has no cancel button for running sessions
- Desktop shows `lastRun` but not active running sessions or their cancel buttons

### Tasks

1. **Add active session tracking** to `desktop/App.jsx` — poll `GET /api/sessions` and show running sessions separately with a Cancel button.
2. **Wire Cancel button** to `POST /api/sessions/{session_id}/cancel`; on success, clear the running state and show "cancelled" status.
3. **Show running session state** with a pulsing indicator and cancel affordance in the metrics bar.
4. **Handle the case** where the session completes or fails while cancel is in-flight (refresh after cancel).
5. **Add `activePane === "cancel"` history** to show cancelled sessions in the session explorer.

### Exit Criteria

- Running sessions appear with a cancel affordance
- Cancel stops the session within 1-2 polling intervals
- Cancel state is reflected in the session history

---

## Step 5: Desktop Parity — Remaining UI Polish

### Context

- `desktop/App.jsx` has all major sections but needs UX completeness
- The `agents` tab in Session Explorer only shows `agentGraph` (nodes) — no agent cards
- No streaming progress indicator
- No model provider selector when creating agents

### Tasks

1. **Agents tab** — show actual saved agents from `trace.agents` with their model/provider info, not just the graph nodes.
2. **Model provider selector** in Agent Builder — fetch `GET /api/models` and show available models for the selected provider (Ollama, OpenAI-compatible) as a dropdown.
3. **Streaming progress indicator** — when a run is in progress, show a pulsing/changing indicator in the metrics bar (iteration count, "streaming..." label).
4. **Better empty states** for all panels.
5. **Keyboard shortcuts**: Escape closes modal, Ctrl+Enter submits run form.

### Exit Criteria

- Agent Builder shows real available models from the server
- Running sessions show iteration progress
- All panels have reasonable empty-state text

---

## Step 6: Runtime Hardening — Session Limits, Shutdown, MCP Health

### Context

- `AgentRuntime` has no session concurrency limit — N concurrent `run()` calls all proceed
- `MCP_SESSION_POOL` is unbounded (`OnceLock<Mutex<HashMap>>`)
- No graceful shutdown — dropping the runtime mid-run doesn't clean up MCP sessions
- Server's `axum::serve` has no graceful shutdown signal handling

### Tasks

1. **Bounded MCP session pool**:
   - Add a configurable `max_mcp_servers` (default: 10) to the pool
   - When at limit, evict the least-recently-used idle session before adding a new one
   - Track `last_used_at` on each `McpSession`
2. **MCP health check**:
   - Add `check_mcp_session(&McpSession) -> bool` — calls a lightweight method (e.g., `tools/list` with a timeout of 3s)
   - On pool miss (key present but process dead), remove from pool automatically
   - Expose `GET /api/mcp/servers/{id}/health` endpoint
3. **Bounded concurrent sessions**:
   - Add `max_concurrent_sessions: usize` to `AgentRuntime` config (default: 4)
   - Use a `Semaphore` to limit concurrent `run()` calls
   - Return `Agent1Error::Runtime("concurrent session limit reached")` when at capacity
4. **Graceful shutdown**:
   - `axum::serve` with `Server::bind(listener).serve(app).with_graceful_shutdown(tokio::signal::ctrl_c())`
   - On shutdown signal: stop accepting new requests, drain existing requests with a timeout (30s)
   - Kill all active MCP child processes on shutdown
5. **Persist `max_iterations` enforcement** — ensure no runaway loops even if the check is bypassed somehow (already implemented, verify).

### Exit Criteria

- MCP pool evicts LRU sessions when at capacity
- Dead MCP processes are detected and removed from pool within 1 health check interval
- Server rejects new sessions with a clear error when at concurrent capacity
- `Ctrl+C` cleanly terminates MCP child processes

---

## Step 7: Streaming Parity — OpenAI-Compatible Streaming + SSE UI

### Context

- `OllamaProvider::chat_stream` — full SSE → chunk collection ✅
- `OpenAiCompatibleProvider` — `chat_stream` falls back to `chat()` (non-streaming) ❌
- No SSE endpoint for UI streaming — the JS polls REST

### Tasks

1. **OpenAI-compatible streaming**:
   - Parse SSE from `/v1/chat/completions` with `stream: true`
   - Handle `data: [DONE]` and `data: ` prefixes
   - Collect chunks into `ChatStreamResponse { content, chunks }`
2. **Add SSE endpoint** `GET /api/sessions/{id}/stream`:
   - Returns `text/event-stream` 
   - Emits events: `iteration_start`, `chunk`, `tool_call`, `tool_result`, `final`
   - Uses `axum::response::sse` or manual `EventStream` type
3. **Desktop SSE client**:
   - Replace polling in `runAgent` with SSE connection to `GET /api/sessions/{id}/stream`
   - Show live chunks in the `run-result` pre element as they arrive
   - Handle `final` event to show complete answer
4. **Add streaming toggle** — `stream: true/false` parameter to `POST /api/sessions/run`

### Exit Criteria

- OpenAI-compatible provider sends `stream: true` and handles SSE response
- Desktop UI shows streamed tokens as they arrive during a run
- SSE endpoint closes cleanly when run completes

---

## Step 8: Integration Tests — API, MCP, Memory, Security

### Context

- `agent1-runtime` has 5 integration tests with in-memory SQLite
- `agent1-cli` has 4 unit tests for TOML parsing
- `agent1-models` has 6 tests with a mock TCP server
- **No integration tests for**: HTTP endpoints, MCP interactions, memory operations, permission enforcement
- **No security tests**

### Tasks

1. **API integration tests** (new `agent1-cli/tests/api.rs`):
   - Spawn `run_server` in a background task on a random port
   - Test all routes: `GET/POST /api/agents`, `GET /api/sessions`, `POST /api/sessions/run`, `GET /api/sessions/{id}/trace`, `POST /api/sessions/{id}/cancel`, `GET /api/mcp/servers`, `GET /api/models`, `GET /api/approvals`, `WS /ws/events`
   - Use `reqwest` to make HTTP calls; `tokio::net::TcpListener` for the server
2. **MCP integration tests** (in `agent1-runtime`):
   - Add a test using a real stdio MCP server (e.g., a minimal shell script that responds to `tools/list` and `tools/call`)
   - Test the health check path
   - Test pool eviction under pressure
3. **Memory integration tests** (new `agent1-db/tests/memory.rs`):
   - Test `search_memories` with various query patterns
   - Test memory write/read cycle through the runtime
   - Test importance sorting
4. **Permission/security tests** (new `agent1-runtime/tests/security.rs`):
   - Agent with `file_write = "deny"` cannot write even if tool is configured
   - Path escape attempts via `file_read` with `../` are blocked
   - MCP server disabled = no tool calls succeed
   - API auth token enforcement when configured
5. **Concurrent session limit test**:
   - Launch N concurrent runs where N > limit
   - Verify early ones complete and later ones get a clear error

### Exit Criteria

- `cargo test` in all crates passes
- New integration test files are part of `cargo test` output
- No `unsafe`, no `TODO`, no `expect("placeholder")` in test code

---

## Step 9: E2E Tests + Security Audit

### Context

- No E2E tests for the desktop UI
- No automated security tests
- No test for the full run loop (start server → create agent → run session → verify result)

### Tasks

1. **Desktop E2E** (new `desktop/e2e/` using Playwright):
   - Test: open desktop app → set API base → save agent → run agent → see result appear
   - Test: load a trace file → verify trace views populate
   - Test: approve a tool call from the UI → verify agent proceeds
   - Test: cancel a running session → verify status updates
   - Test: add an MCP server → verify it appears in the list
   - Run against the built app (`npm run tauri:build` → binary) or dev mode
2. **Secrets redaction test**:
   - Verify that `redact_secrets_text` and `redact_secrets_value` handle all patterns in `agent1-core`
   - Add a fuzz test for `redact_secrets_text` with adversarial inputs
3. **Path escape test**:
   - Unit test for `PathEscapesWorkspace` covering `../`, absolute paths, Windows paths
4. **Approval bypass test**:
   - Verify that a tool configured with `ask` always prompts, never auto-allows
5. **Security smoke test**:
   - Server bind address enforcement (must be loopback)
   - API token required when configured
   - CORS headers only allow loopback origins

### Exit Criteria

- Playwright E2E tests run against the desktop app and pass
- Security tests pass and cover the listed cases
- No security findings in the test suite itself

---

## Step 10: CI Setup

### Context

- No `.github/workflows/` exists yet
- The project has a Rust workspace + a Node.js/Tauri desktop

### Tasks

1. **Create `.github/workflows/ci.yml`**:
   - Trigger: push to `main`, PRs
   - Steps:
     - Checkout
     - **Rust**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --all`
     - **Desktop**: `cd desktop && npm install && npm run build && npm test` (or skip test if not written yet)
   - Cache cargo and node_modules
2. **Add `rustfmt.toml`** if not present — `edition = "2024"`, `newline_style = "unix"`
3. **Add `.cargo/config.toml`** with `[build] rustflags = ["-C", "instrument-coverage"]` for coverage (optional — only if coverage tooling is set up)
4. **Verify CI passes** on a test PR before merging

### Exit Criteria

- CI workflow file exists and is valid YAML
- `cargo test --all` passes in CI environment
- Desktop build passes in CI

---

## Step 11: Milestone 10 — Installer and Distribution

### Context

- Tauri builds a `.exe` on Windows but there's no installer
- No MSI/NSIS/InnoSetup setup
- No publish to GitHub Releases workflow

### Tasks

1. **Tauri Windows installer**:
   - Configure `tauri.conf.json` with `bundle.active.tauri.bundleWindows` targets: `nsis` (or `wix` if preferred)
   - Add installer configuration: name, icon, shortcuts
   - Add `POST /api/sessions/run` → background task that streams progress
2. **Create `agent1-install.ps1`** PowerShell script:
   - Downloads latest release binary for the platform
   - Sets up `%LOCALAPPDATA%\Agent1` directory
   - Creates a start menu shortcut
   - Shows completion message
3. **Create GitHub Release workflow** `.github/workflows/release.yml`:
   - Trigger: new git tag matching `v*`
   - Build Tauri app for Windows (and macOS/Linux if applicable)
   - Attach `.exe` installer to GitHub Release
   - Run smoke test against the built binary
4. **Version bump** to `0.2.0` in all `Cargo.toml` files and `desktop/package.json`

### Exit Criteria

- `npm run tauri:build` produces a Windows installer (`.msi` or `.exe` NSIS)
- `agent1-install.ps1` can download and install the latest release
- Release workflow builds and attaches artifacts to GitHub Release

---

## Step 12: Documentation Polish and Final Smoke Tests

### Context

- README exists but reflects the old `app/` flow
- No examples directory with runnable scenarios
- No security disclosure process documented

### Tasks

1. **Update `README.md`**:
   - Remove references to `app/` static shell
   - Update quick start to use Tauri desktop
   - Add section: "First Run" with agent creation, running a task
   - Document all CLI commands
   - Add troubleshooting section (common Ollama connection issues, MCP server errors)
2. **Create `docs/` content**:
   - `docs/agents.md` — how to write an agent TOML file
   - `docs/mcp.md` — how to register and use MCP servers
   - `docs/api.md` — full API reference (endpoints, request/response shapes)
   - `docs/security.md` — permission model, secrets redaction, trust boundaries
3. **Smoke test script** `scripts/smoke-test.ps1`:
   - Starts the server
   - Creates a test agent
   - Runs a simple task (`echo "hello"`)
   - Verifies output appears
   - Checks no panic logs
4. **Final `cargo test` pass** on the complete codebase

### Exit Criteria

- README reflects the shipped product
- `docs/` has at least the 4 reference documents listed
- Smoke test script runs cleanly against a fresh install

---

## Rollback Strategy

If a step introduces regressions:
- **Steps 1-5** (UI work): Revert the specific component change; `cargo test --all` must still pass
- **Step 6** (runtime): Run `cargo test -p agent1-runtime` — if tests fail, the MCP pool or session limit code needs fixes before proceeding
- **Steps 8-9** (tests): Tests must be added in the same PR as the feature they test; never merge untested features
- **Step 10** (CI): CI must pass before any PR is merged

## Parallel Execution

Steps **2, 3, 4, 5** are independent and can run in parallel with separate agents:
- Agent A: Step 2 (MCP manager)
- Agent B: Step 3 (Memory viewer)
- Agent C: Step 4 (Session cancel)
- Agent D: Step 5 (Desktop polish)

Steps **8** and **9** can also partially overlap (Agent E: API integration tests while Agent D finishes Step 5).

Steps **10, 11, 12** are sequential post-tests.

## Model Tier Assignments

- Steps 1, 6, 7: **Strongest model** (architectural decisions — MCP pool design, graceful shutdown, SSE architecture)
- Steps 2, 3, 4, 5: **Default model** (feature implementation — well-defined, bounded scope)
- Steps 8, 9: **Strongest model** (security test design — adversarial thinking)
- Steps 10, 11, 12: **Default model** (CI config, docs — straightforward)

## Anti-Patterns to Avoid

1. **Don't embed the Axum server in the Tauri process** — keep them separate for Milestone 10; complexity is not worth it yet
2. **Don't add streaming to the desktop UI until the SSE endpoint exists** — ensure Step 7's backend is stable before wiring desktop JS
3. **Don't write tests for features not yet merged** — sequential ordering prevents test maintenance debt
4. **Don't skip the session cancel check timing** — the current `Cancelled` check only fires between iterations; if a model call takes 5 minutes, cancellation takes up to 5 minutes. This is a known limitation, document it rather than trying to fix it in this milestone
5. **Don't remove `app/` static file serving until the redirect is in place** — avoid breaking the `agent1 server` flow for users who start it without the desktop

## Summary

- **12 steps** total
- **~4 sessions** for parallel steps (2+3+4+5 can run simultaneously)
- **~6 sessions** estimated total (3 parallel sessions + 6 sequential + review/adjustments)
- Hardest step: **Step 7** (streaming parity — SSE + OpenAI streaming both need care)
- Most security-sensitive step: **Step 8** (integration tests with real MCP, permission enforcement)