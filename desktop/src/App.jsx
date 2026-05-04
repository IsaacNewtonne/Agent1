import { useEffect, useMemo, useRef, useState } from "react";

const SETTINGS_KEY = "agent1_desktop_settings_v1";

function defaultSettings() {
  return {
    apiBase: "http://127.0.0.1:17371",
    refreshMs: 2000,
    autoRefresh: true,
  };
}

function loadSettings() {
  try {
    const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}");
    const defaults = defaultSettings();
    return {
      apiBase: saved.apiBase || defaults.apiBase,
      refreshMs: Number(saved.refreshMs) >= 500 ? Number(saved.refreshMs) : defaults.refreshMs,
      autoRefresh: saved.autoRefresh !== false,
    };
  } catch {
    return defaultSettings();
  }
}

async function fetchJson(base, path, options = {}) {
  const response = await fetch(`${base}${path}`, options);
  if (!response.ok) throw new Error(`${path} returned ${response.status}`);
  return response.json();
}

function getAgentInitials(name = "") {
  return name
    .split(" ")
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
}

function getServerColor(name = "") {
  const n = name.toLowerCase();
  if (n.includes("filesystem") || n.includes("file")) return "var(--accent-secondary)";
  if (n.includes("git")) return "var(--accent-secondary)";
  if (n.includes("slack")) return "var(--accent-purple)";
  if (n.includes("web")) return "var(--accent-primary)";
  return "var(--text-secondary)";
}

export default function App() {
  const [settings, setSettings] = useState(loadSettings);
  const [trace, setTrace] = useState({
    session: {},
    messages: [],
    tool_calls: [],
    events: [],
    approvals: [],
    agents: [],
    mcp_servers: [],
    model_providers: [],
    memories: [],
  });
  const [runForm, setRunForm] = useState({
    agentId: "",
    input: "",
    workspace: ".",
  });
  const [lastRun, setLastRun] = useState(null);
  const [activeSessions, setActiveSessions] = useState([]);
  const [status, setStatus] = useState("");
  const [wsState, setWsState] = useState("disconnected");
  const [selectedAgentId, setSelectedAgentId] = useState(null);
  const [isRunning, setIsRunning] = useState(false);
  const [agentTab, setAgentTab] = useState("all");
  const [expandedAgents, setExpandedAgents] = useState(new Set());
  const [expandedServers, setExpandedServers] = useState(new Set());
  const [selectedApprovalId, setSelectedApprovalId] = useState(null);
  const [expandedEvents, setExpandedEvents] = useState(new Set());
  const runFormRef = useRef(null);
  const wsRef = useRef(null);
  const eventsEndRef = useRef(null);

  const pendingApprovals = useMemo(
    () => (trace.approvals || []).filter((item) => !item.decision),
    [trace.approvals]
  );
  const recentEvents = useMemo(() => (trace.events || []).slice(-20).reverse(), [trace.events]);
  const streamingOutput = useMemo(() => {
    const msgs = (trace.messages || []).filter((m) => m.role === "assistant");
    return msgs.length > 0 ? msgs[msgs.length - 1].content : "";
  }, [trace.messages]);

  const filteredAgents = useMemo(() => {
    const agents = trace.agents || [];
    if (agentTab === "all") return agents;
    const runningIds = new Set(activeSessions.map((s) => s.root_agent_id));
    if (agentTab === "running") return agents.filter((a) => runningIds.has(a.id));
    if (agentTab === "idle") return agents.filter((a) => !runningIds.has(a.id));
    return agents;
  }, [trace.agents, activeSessions, agentTab]);

  async function refreshAll() {
    try {
      const [agents, sessions, events, mcpServers, approvals, models] = await Promise.all([
        fetchJson(settings.apiBase, "/api/agents"),
        fetchJson(settings.apiBase, "/api/sessions"),
        fetchJson(settings.apiBase, "/api/events"),
        fetchJson(settings.apiBase, "/api/mcp/servers"),
        fetchJson(settings.apiBase, "/api/approvals"),
        fetchJson(settings.apiBase, "/api/models"),
      ]);
      const latestSession = (sessions.sessions || [])[0];
      let baseTrace = { session: {}, messages: [], tool_calls: [], events: [], approvals: [] };
      if (latestSession) {
        try {
          baseTrace = await fetchJson(
            settings.apiBase,
            `/api/sessions/${encodeURIComponent(latestSession.id)}/trace`
          );
        } catch {
          // fallback to empty trace
        }
      }
      setTrace({
        ...baseTrace,
        events: baseTrace.events?.length ? baseTrace.events : events.events || [],
        approvals: baseTrace.approvals || approvals.approvals || [],
        agents: agents.agents || [],
        mcp_servers: mcpServers.servers || [],
        model_providers: models.providers || [],
      });
      const running = (sessions.sessions || []).filter((s) => s.status === "running");
      setActiveSessions(running);
    } catch (error) {
      setStatus(`Refresh failed: ${error.message}`);
    }
  }

  useEffect(() => {
    refreshAll().catch((error) => setStatus(`Refresh failed: ${error.message}`));
  }, [settings.apiBase]);

  useEffect(() => {
    if (!settings.autoRefresh) return undefined;
    const timer = setInterval(refreshAll, settings.refreshMs);
    return () => clearInterval(timer);
  }, [settings.autoRefresh, settings.refreshMs, settings.apiBase]);

  useEffect(() => {
    const wsBase = settings.apiBase.replace(/^http/i, "ws");
    const ws = new WebSocket(`${wsBase}/ws/events`);
    wsRef.current = ws;
    setWsState("connecting");

    ws.onopen = () => setWsState("connected");
    ws.onclose = () => setWsState("disconnected");
    ws.onerror = () => setWsState("error");
    ws.onmessage = (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch {
        return;
      }
      if (message.type === "event" && message.event) {
        setTrace((current) => {
          const events = current.events || [];
          if (events.some((item) => item.id === message.event.id)) return current;
          return { ...current, events: [...events, message.event] };
        });
      }
      if (message.type === "approval_decision_ack") {
        setStatus(`Approval ${message.approval_id} -> ${message.decision}`);
        refreshAll().catch(() => {});
      }
    };
    return () => {
      wsRef.current = null;
      ws.close();
    };
  }, [settings.apiBase]);

  useEffect(() => {
    if (eventsEndRef.current) {
      eventsEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [recentEvents]);

  function saveSettings(event) {
    event.preventDefault();
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    setStatus("Settings saved.");
    refreshAll().catch((error) => setStatus(`Refresh failed: ${error.message}`));
  }

  async function runAgent(event) {
    event.preventDefault();
    const payload = {
      agent_id: runForm.agentId.trim(),
      input: runForm.input.trim(),
      workspace: runForm.workspace.trim() || ".",
    };
    if (!payload.agent_id || !payload.input) {
      setStatus("Run blocked: agent and task input are required.");
      return;
    }
    setIsRunning(true);
    setSelectedAgentId(payload.agent_id);
    try {
      const result = await fetchJson(settings.apiBase, "/api/sessions/run", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setLastRun({ sessionId: result.session_id || "", final: result.final || "" });
      setStatus(`Run completed for ${payload.agent_id}`);
      await refreshAll();
    } catch (error) {
      setStatus(`Run failed: ${error.message}`);
    } finally {
      setIsRunning(false);
    }
  }

  async function cancelSession(sessionId) {
    try {
      await fetchJson(settings.apiBase, `/api/sessions/${encodeURIComponent(sessionId)}/cancel`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({}),
      });
      setStatus(`Cancelled session ${sessionId}`);
      await refreshAll();
    } catch (error) {
      setStatus(`Cancel failed: ${error.message}`);
    }
  }

  async function decideApproval(approvalId, decision) {
    try {
      const ws = wsRef.current;
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: "approval_decision", approval_id: approvalId, decision }));
        return;
      }
      await fetchJson(settings.apiBase, `/api/tool-approvals/${encodeURIComponent(approvalId)}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ decision }),
      });
      await refreshAll();
    } catch (error) {
      setStatus(`Approval update failed: ${error.message}`);
    }
  }

  function toggleAgentExpand(agentId) {
    setExpandedAgents((prev) => {
      const next = new Set(prev);
      if (next.has(agentId)) next.delete(agentId);
      else next.add(agentId);
      return next;
    });
  }

  function toggleServerExpand(serverId) {
    setExpandedServers((prev) => {
      const next = new Set(prev);
      if (next.has(serverId)) next.delete(serverId);
      else next.add(serverId);
      return next;
    });
  }

  function toggleEventExpand(eventId) {
    setExpandedEvents((prev) => {
      const next = new Set(prev);
      if (next.has(eventId)) next.delete(eventId);
      else next.add(eventId);
      return next;
    });
  }

  const isAgentRunning = (agentId) => activeSessions.some((s) => s.root_agent_id === agentId);
  const runningAgentIds = useMemo(() => new Set(activeSessions.map((s) => s.root_agent_id)), [activeSessions]);

  return (
    <div className="app-layout">
      <div className="atmosphere" />

      {/* ===== LEFT PANEL ===== */}
      <aside className="panel left-panel">
        <div className="section">
          <div className="section-title">Connection</div>
          <div className="connection-status">
            <span className={`status-dot ${wsState === "connected" ? "connected" : "disconnected"}`} />
            <span className="connection-url">{settings.apiBase.replace("http://", "")}</span>
          </div>
        </div>

        <div className="section">
          <div className="section-title">Settings</div>
          <form className="settings-form" onSubmit={saveSettings}>
            <div className="form-group">
              <label>API Base</label>
              <input
                type="text"
                value={settings.apiBase}
                onChange={(e) => setSettings((s) => ({ ...s, apiBase: e.target.value }))}
              />
            </div>
            <div className="form-group">
              <label>Refresh (ms)</label>
              <input
                type="number"
                min={500}
                value={settings.refreshMs}
                onChange={(e) => setSettings((s) => ({ ...s, refreshMs: Number(e.target.value) }))}
              />
            </div>
            <div className="form-row">
              <label>
                <input
                  type="checkbox"
                  checked={settings.autoRefresh}
                  onChange={(e) => setSettings((s) => ({ ...s, autoRefresh: e.target.checked }))}
                />
                Auto-refresh
              </label>
            </div>
            <button type="submit" className="btn btn-ghost btn-sm">Save</button>
          </form>
        </div>

        <div className="section">
          <div className="section-title">Local Agents</div>
          <div className="tab-filter">
            <button className={agentTab === "all" ? "active" : ""} onClick={() => setAgentTab("all")}>All</button>
            <button className={agentTab === "running" ? "active" : ""} onClick={() => setAgentTab("running")}>Running</button>
            <button className={agentTab === "idle" ? "active" : ""} onClick={() => setAgentTab("idle")}>Idle</button>
          </div>
          <div className="agent-list">
            {filteredAgents.length === 0 ? (
              <p className="muted empty-state">No agents yet</p>
            ) : (
              filteredAgents.map((agent) => (
                <div
                  key={agent.id}
                  className={`agent-item ${selectedAgentId === agent.id ? "selected" : ""}`}
                  onClick={() => {
                    setSelectedAgentId(agent.id);
                    setRunForm((f) => ({ ...f, agentId: agent.id }));
                  }}
                >
                  <div
                    className="avatar"
                    style={{
                      background: runningAgentIds.has(agent.id)
                        ? "linear-gradient(135deg, rgba(255,179,71,0.2), rgba(255,179,71,0.1))"
                        : "linear-gradient(135deg, rgba(44,224,163,0.2), rgba(44,224,163,0.1))",
                      border: runningAgentIds.has(agent.id)
                        ? "1px solid rgba(255,179,71,0.4)"
                        : "1px solid rgba(44,224,163,0.4)",
                      color: runningAgentIds.has(agent.id) ? "var(--accent-warning)" : "var(--accent-primary)",
                    }}
                  >
                    {getAgentInitials(agent.name)}
                  </div>
                  <div className="agent-info">
                    <div className="agent-name">{agent.name}</div>
                    <div className="agent-model">
                      {agent.model?.provider}/{agent.model?.model}
                    </div>
                  </div>
                  <span className={`status-badge ${runningAgentIds.has(agent.id) ? "running" : "idle"}`}>
                    {runningAgentIds.has(agent.id) ? "running" : "idle"}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>
      </aside>

      {/* ===== CENTER CANVAS ===== */}
      <main className="center-canvas">
        <div className="canvas-header">
          <div className="project-icon">
            <div className="icon-frame">
              <span>A1</span>
            </div>
            <span className="project-name">Agent1</span>
          </div>
        </div>

        <div className="canvas-body">
          {/* Left Tree - Local Agents */}
          <div className="agent-tree left">
            <div className="section-title">Local Agents</div>
            {filteredAgents.length === 0 ? (
              <p className="muted empty-state">No agents configured</p>
            ) : (
              <div className="tree-branch">
                {filteredAgents.map((agent) => (
                  <div key={agent.id} className="tree-node left">
                    <div className="node-header">
                      <div
                        className="node-avatar"
                        style={{
                          background: runningAgentIds.has(agent.id)
                            ? "linear-gradient(135deg, rgba(255,179,71,0.3), rgba(255,179,71,0.1))"
                            : "linear-gradient(135deg, rgba(44,224,163,0.3), rgba(44,224,163,0.1))",
                          border: runningAgentIds.has(agent.id)
                            ? "1px solid rgba(255,179,71,0.5)"
                            : "1px solid rgba(44,224,163,0.5)",
                          color: runningAgentIds.has(agent.id) ? "var(--accent-warning)" : "var(--accent-primary)",
                        }}
                      >
                        {getAgentInitials(agent.name)}
                      </div>
                      <div className="node-info">
                        <div className="node-name">{agent.name}</div>
                        <div className="node-meta">{agent.model?.model}</div>
                      </div>
                      <span className={`node-status ${runningAgentIds.has(agent.id) ? "running" : "idle"}`} />
                    </div>
                    <div className="node-tools" onClick={() => toggleAgentExpand(agent.id)} style={{ cursor: "pointer" }}>
                      {expandedAgents.has(agent.id) ? (
                        <div className="tool-item">− tools</div>
                      ) : (
                        <div className="tool-item">+ {agent.tools?.length || 0} tools</div>
                      )}
                    </div>
                    {expandedAgents.has(agent.id) && (
                      <div className="tree-children">
                        {(agent.tools || []).map((tool, i) => (
                          <div key={i} className="tool-item">{tool}</div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="canvas-divider" />

          {/* Right Tree - External Agents / MCP */}
          <div className="agent-tree right">
            <div className="section-title">External Agents</div>
            {(trace.mcp_servers || []).length === 0 ? (
              <p className="muted empty-state">No MCP servers</p>
            ) : (
              <div className="tree-branch">
                {(trace.mcp_servers || []).map((server) => (
                  <div
                    key={server.id || server.name}
                    className="tree-node right"
                    style={{ borderColor: `${getServerColor(server.name)}40` }}
                  >
                    <div className="node-header">
                      <div
                        className="node-avatar"
                        style={{
                          background: `linear-gradient(135deg, ${getServerColor(server.name)}20, ${getServerColor(server.name)}08)`,
                          border: `1px solid ${getServerColor(server.name)}50`,
                          color: getServerColor(server.name),
                        }}
                      >
                        {getAgentInitials(server.name)}
                      </div>
                      <div className="node-info">
                        <div className="node-name">{server.name}</div>
                        <div className="node-meta">{server.enabled ? "enabled" : "disabled"}</div>
                      </div>
                      <span className={`node-status ${server.enabled ? "active" : "idle"}`} />
                    </div>
                    <div className="node-tools" onClick={() => toggleServerExpand(server.id || server.name)} style={{ cursor: "pointer" }}>
                      {expandedServers.has(server.id || server.name) ? (
                        <div className="tool-item">− {server.tools?.length || 0} tools</div>
                      ) : (
                        <div className="tool-item">+ {server.tools?.length || 0} tools</div>
                      )}
                    </div>
                    {expandedServers.has(server.id || server.name) && (
                      <div className="tree-children">
                        {(server.tools || []).map((tool, i) => (
                          <div key={i} className="tool-item">{tool}</div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Task Input Bar */}
        <div className="task-input-bar">
          <form ref={runFormRef} onSubmit={runAgent}>
            <textarea
              rows={3}
              placeholder="What should the agents do?"
              value={runForm.input}
              onChange={(e) => setRunForm((f) => ({ ...f, input: e.target.value }))}
            />
            <div className="task-input-actions">
              <span className="char-count">{runForm.input.length} chars</span>
              <div style={{ display: "flex", gap: "8px" }}>
                {isRunning && (
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={() => {
                      const running = activeSessions[0];
                      if (running) cancelSession(running.id);
                    }}
                  >
                    Cancel
                  </button>
                )}
                <button
                  type="submit"
                  className={`btn btn-confirm ${isRunning ? "btn-loading" : ""}`}
                  disabled={isRunning || !runForm.agentId || !runForm.input.trim()}
                >
                  {isRunning ? "Running..." : "Run"}
                </button>
              </div>
            </div>
          </form>
          <div style={{ marginTop: "8px" }}>
            <select
              value={runForm.agentId}
              onChange={(e) => setRunForm((f) => ({ ...f, agentId: e.target.value }))}
              style={{
                width: "100%",
                padding: "8px 10px",
                background: "var(--surface-2)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--radius-sm)",
                color: "var(--text-primary)",
                fontFamily: "var(--font-ui)",
                fontSize: "12px",
              }}
            >
              <option value="">Select agent...</option>
              {(trace.agents || []).map((agent) => (
                <option key={agent.id} value={agent.id}>{agent.name}</option>
              ))}
            </select>
          </div>
        </div>
      </main>

      {/* ===== RIGHT PANEL ===== */}
      <aside className="panel right-panel">
        <div className="activity-section">
          <div className="activity-title">
            Active Sessions
            <span className="activity-count">{activeSessions.length}</span>
          </div>
          {activeSessions.length === 0 ? (
            <p className="muted empty-state">No active sessions</p>
          ) : (
            <div className="activity-feed">
              {activeSessions.map((session) => (
                <div key={session.id} className="activity-event">
                  <div className="event-header">
                    <span className="event-time">{session.id?.slice(0, 8)}...</span>
                    <span className="event-type">running</span>
                  </div>
                  <div className="event-payload">{session.title || session.root_agent_id}</div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="activity-section">
          <div className="activity-title">Stream Output</div>
          <div className={`stream-output ${isRunning ? "" : "idle"}`}>
            {streamingOutput || (isRunning ? "Waiting for output..." : "No output yet")}
          </div>
        </div>

        <div className="activity-section">
          <div className="activity-title">
            Recent Events
            <span className="activity-count">{recentEvents.length}</span>
          </div>
          <div className="activity-feed">
            {recentEvents.map((event) => (
              <div
                key={event.id || `${event.created_at}-${event.event_type}`}
                className={`activity-event ${event.event_type?.includes("error") ? "error" : ""} ${
                  expandedEvents.has(event.id) ? "expanded" : ""
                }`}
                onClick={() => toggleEventExpand(event.id)}
              >
                <div className="event-header">
                  <span className="event-time">{event.created_at?.slice(11, 19) || "—"}</span>
                  <span className={`event-type ${event.event_type?.includes("error") ? "error" : ""}`}>
                    {event.event_type?.replace(/([A-Z])/g, " $1").trim() || "event"}
                  </span>
                </div>
                <div className="event-payload">
                  {expandedEvents.has(event.id)
                    ? JSON.stringify(event.payload || {}, null, 2)
                    : JSON.stringify(event.payload || {}).slice(0, 60) + "..."}
                </div>
              </div>
            ))}
            <div ref={eventsEndRef} />
          </div>
        </div>

        <div className="activity-section">
          <div className="activity-title">
            Pending Approvals
            <span className="activity-count">{pendingApprovals.length}</span>
          </div>
          {pendingApprovals.length === 0 ? (
            <p className="muted empty-state">No pending approvals</p>
          ) : (
            <div>
              {pendingApprovals.map((approval) => (
                <div key={approval.id} className="approval-item">
                  <div className="approval-header">{approval.id}</div>
                  <div className="approval-request">
                    {approval.request?.tool_name || approval.request?.tool || "tool"} —{" "}
                    {approval.agent_id}
                  </div>
                  <div className="approval-actions">
                    <button
                      className="btn btn-confirm btn-sm"
                      onClick={() => decideApproval(approval.id, "approved")}
                    >
                      Approve
                    </button>
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => decideApproval(approval.id, "denied")}
                    >
                      Deny
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </aside>

      {/* Status Bar */}
      <div
        style={{
          position: "fixed",
          bottom: "12px",
          left: "50%",
          transform: "translateX(-50%)",
          padding: "8px 16px",
          background: "var(--surface-1)",
          border: "1px solid var(--border-default)",
          borderRadius: "var(--radius)",
          fontSize: "12px",
          color: "var(--text-secondary)",
          zIndex: 100,
        }}
      >
        {status || "System ready"} · {trace.session?.status || "idle"} · WS {wsState}
      </div>
    </div>
  );
}