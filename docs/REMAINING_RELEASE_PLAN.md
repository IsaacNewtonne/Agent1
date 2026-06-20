# Remaining Release Plan

This file tracks the work that still requires new framework dependencies, packaging tools, or a larger UI build-out.

## Desktop

- Retire the legacy static shell now that the React/Tauri collaboration workspace is active.
- Add dedicated desktop UX for MCP manager, memory browsing/editing, and session cancellation.
- Add desktop E2E tests for create/save/run/approve/session-explorer workflows.
- Add a production diagnostics screen for app version, API version, DB path, provider health, MCP health, and exportable logs.

## Runtime

- Extend streaming model responses beyond Ollama baseline and add provider parity/fallback behavior.
- Harden managed MCP stdio process pool with health checks, bounded sessions, and graceful shutdown.
- Add long-running run cancellation with task handles, child-process cleanup, and event propagation.

## Verification

- Add integration tests for HTTP API, MCP lifecycle, memory storage, and security policy paths.
- Add release smoke tests for install-and-first-run under 10 minutes.
- Keep `scripts/smoke-test.ps1` in CI as the server/API vertical-slice guard.
