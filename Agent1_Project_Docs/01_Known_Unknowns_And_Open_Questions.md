# Known Unknowns and Open Questions

## Model Behavior

- How reliable are local models at structured tool calling?
- Which small models work best for tool routing?
- Should Agent1 use JSON mode where available?
- Should the runtime enforce a repair loop for malformed tool calls?

## MCP

- Which MCP transport should be prioritized after stdio?
- Should MCP resources be exposed as memory sources?
- Should users be able to create MCP server profiles?

## A2A

- How close should Agent1 stay to emerging remote agent protocols?
- Should the local agent card format exactly mirror external formats or remain simpler?
- Should remote agent calling be disabled until security model matures?

## Memory

- Should memory writes require approval by default?
- Should vector search be included in MVP or delayed?
- Which embedding backend should be used locally?

## UI

- Should the first UI use React or Svelte?
- How much of the trace should be shown to normal users by default?
- Should the graph be live during execution or only post-run?

## Security

- How strict should shell command allowlists be?
- Should file writes be patch-only in MVP?
- Should Agent1 include a temporary workspace sandbox per run?

## Packaging

- Should Docker be included in MVP or after desktop release?
- Should Windows be the first official target?
- Should the app bundle Ollama detection helpers?
