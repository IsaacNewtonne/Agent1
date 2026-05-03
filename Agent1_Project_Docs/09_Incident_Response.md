# Incident Response

## Incident Types

| Type | Example |
|---|---|
| Security | Agent ran unsafe command |
| Data loss | Agent overwrote file |
| Privacy | Secret appeared in logs |
| Reliability | Runtime crashed |
| MCP | Malicious or broken MCP server |
| Model | Model generated unsafe tool call |

## Immediate Response Steps

1. Stop active run.
2. Disable dangerous tools.
3. Preserve logs and session trace.
4. Identify affected files/data.
5. Export incident session.
6. Apply mitigation.
7. Document root cause.

## Unsafe Shell Command

Steps:

1. Cancel active run.
2. Disable shell tool.
3. Inspect command event.
4. Check filesystem impact.
5. Restore from backup if needed.
6. Add command to denylist.

## File Damage

Steps:

1. Stop Agent1.
2. Identify file write events.
3. Recover from backup or artifact history.
4. Review permission policy.
5. Consider patch-only write mode.

## Secret Leak

Steps:

1. Delete affected logs/exports.
2. Rotate exposed secret.
3. Add redaction pattern.
4. Create regression test.
5. Review related tool outputs.

## MCP Incident

Steps:

1. Disable MCP server.
2. Review MCP calls.
3. Remove unsafe tool.
4. Report issue if third-party.
5. Add warning to config.

## Post-Incident Report

Include:

- Incident summary
- Time discovered
- Affected data
- Root cause
- Mitigation
- Preventive action
- Test added
