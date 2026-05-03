# Risk Register and Assumptions

## Assumptions

- Users can install or run Ollama or another local model provider.
- Local models may be weaker than hosted frontier models.
- Users want privacy and control more than maximum model intelligence.
- MCP and A2A-style patterns will remain useful.
- Rust is acceptable even if some AI libraries are less mature than Python equivalents.

## Risks

| ID | Risk | Impact | Likelihood | Mitigation |
|---|---|---:|---:|---|
| R-001 | Local models produce poor tool calls | High | Medium | Add strict tool schemas, examples, retries, and planner prompts |
| R-002 | Shell tool damages user system | Critical | Medium | Default ask/deny, allowlists, workspace sandbox, command preview |
| R-003 | MCP server is malicious or buggy | High | Medium | Treat MCP as untrusted, require user approval, log calls |
| R-004 | UI becomes complex | Medium | High | Build CLI/runtime first, keep UI task-focused |
| R-005 | Rust MCP/A2A ecosystem changes | Medium | Medium | Isolate protocol adapters |
| R-006 | SQLite database grows large | Medium | Medium | Add retention settings, indexes, export/archive |
| R-007 | Users expect cloud-level intelligence | Medium | High | Make local model limitations clear |
| R-008 | Prompt injection through files | High | Medium | Add tool boundaries and file-source warnings |
| R-009 | Agent loops indefinitely | High | Medium | Max iterations, timeouts, cancellation |
| R-010 | Secrets leak into logs | Critical | Low | Redaction layer and secret handling rules |

## Risk Controls

- Permission guard
- Tool timeouts
- Event logs
- Workspace restrictions
- Redaction
- Memory approval settings
- Max iteration limits
- Human approval for destructive actions
