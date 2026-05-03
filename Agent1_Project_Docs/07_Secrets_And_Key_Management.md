# Secrets and Key Management

## Principle

Agent1 should not require secrets for MVP because it is local-first.

However, users may configure optional local or remote integrations later. Secret handling must be safe from the start.

## Secret Types

- API keys
- Local auth tokens
- MCP server credentials
- SSH keys
- Database passwords
- Personal access tokens

## Storage Rules

- Do not store raw secrets in agent configs.
- Do not log secrets.
- Do not include secrets in session exports by default.
- Use OS keychain where available.
- Fallback to encrypted local secret store if needed.
- For MVP, prefer environment variable references.

## Secret Reference Format

```toml
[models.custom]
base_url = "http://localhost:8000/v1"
api_key_ref = "env:LOCAL_MODEL_API_KEY"
```

Allowed reference types:

```text
env:NAME
keychain:NAME
file:PATH
```

## Redaction

Redaction must run on:

- Tool input logs
- Tool output logs
- Model request logs
- Model response logs
- Error messages
- Exported traces

## Redaction Patterns

Redact likely:

- `sk-*`
- `ghp_*`
- `-----BEGIN PRIVATE KEY-----`
- `password=...`
- `token=...`
- `.env` values

## Developer Rules

- Never print all environment variables.
- Never send secrets to model unless user explicitly approves.
- Never expose secrets in UI event feed.
- Never include secrets in exported Markdown.
