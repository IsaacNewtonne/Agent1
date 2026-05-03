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

async function fetchJson(base, path, options) {
  const response = await fetch(`${base}${path}`, options);
  if (!response.ok) throw new Error(`${path} returned ${response.status}`);
  return response.json();
}

function safeText(value, fallback = "-") {
  return value || fallback;
}

function collectAgentGraph(trace) {
  const ids = new Set([trace.session?.root_agent_id].filter(Boolean));
  (trace.messages || []).forEach((message) => {
    if (message.from_agent_id) ids.add(message.from_agent_id);
    if (message.to_agent_id) ids.add(message.to_agent_id);
  });
  (trace.events || []).forEach((event) => {
    if (event.agent_id) ids.add(event.agent_id);
  });
  (trace.tool_calls || []).forEach((call) => {
    if (call.agent_id) ids.add(call.agent_id);
  });
  return Array.from(ids).map((id) => ({
    id,
    role: id === trace.session?.root_agent_id ? "root" : "participant",
  }));
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
  const [agentForm, setAgentForm] = useState({
    id: "assistant_local",
    name: "Assistant Local",
    provider: "ollama",
    model: "qwen2.5:7b",
    prompt: "You are a practical local agent.",
  });
  const [runForm, setRunForm] = useState({
    agentId: "assistant_local",
    input: "Review the current workspace status and summarize key risks.",
    workspace: ".",
  });
  const [lastRun, setLastRun] = useState(null);
  const [activeSessions, setActiveSessions] = useState([]);
  const [status, setStatus] = useState("");
  const [wsState, setWsState] = useState("disconnected");
  const [activePane, setActivePane] = useState("events");
  const [selectedApprovalId, setSelectedApprovalId] = useState(null);
  const [isRunning, setIsRunning] = useState(false);
  const [memorySearch, setMemorySearch] = useState("");
  const [memoryItems, setMemoryItems] = useState([]);
  const [memoryForm, setMemoryForm] = useState({
    content: "",
    scope: "agent",
    tags: "",
    importance: 0,
  });
  const [mcpForm, setMcpForm] = useState({
    name: "",
    command: "",
    args: "",
    enabled: true,
  });
  const [mcpToolsModal, setMcpToolsModal] = useState(null);
  const [mcpTools, setMcpTools] = useState([]);
  const [allSessions, setAllSessions] = useState([]);
  const [viewTraceSession, setViewTraceSession] = useState(null);
  const runFormRef = useRef(null);
  const wsRef = useRef(null);

  const pendingApprovals = useMemo(
    () => (trace.approvals || []).filter((item) => !item.decision),
    [trace.approvals],
  );
  const recentEvents = useMemo(() => (trace.events || []).slice(-40).reverse(), [trace.events]);
  const recentMessages = useMemo(() => (trace.messages || []).slice(-30).reverse(), [trace.messages]);
  const recentToolCalls = useMemo(
    () => (trace.tool_calls || []).slice(-30).reverse(),
    [trace.tool_calls],
  );
  const agentGraph = useMemo(() => collectAgentGraph(trace), [trace]);
  const selectedApproval = useMemo(
    () => (trace.approvals || []).find((item) => item.id === selectedApprovalId) || null,
    [trace.approvals, selectedApprovalId],
  );
  const sessionMetrics = useMemo(() => {
    const events = trace.events || [];
    const toolCalls = trace.tool_calls || [];
    return {
      messages: (trace.messages || []).length,
      events: events.length,
      tools: toolCalls.length,
      errors: events.filter((event) => (event.event_type || "").includes("error")).length,
    };
  }, [trace.messages, trace.events, trace.tool_calls]);

  async function refreshAll() {
    const [agents, sessions, events, mcpServers, approvals, models] = await Promise.all([
      fetchJson(settings.apiBase, "/api/agents"),
      fetchJson(settings.apiBase, "/api/sessions"),
      fetchJson(settings.apiBase, "/api/events"),
      fetchJson(settings.apiBase, "/api/mcp/servers"),
      fetchJson(settings.apiBase, "/api/approvals"),
      fetchJson(settings.apiBase, "/api/models"),
    ]);
    const latestSession = (sessions.sessions || [])[0];
    const baseTrace = latestSession
      ? await fetchJson(
          settings.apiBase,
          `/api/sessions/${encodeURIComponent(latestSession.id)}/trace`,
        )
      : { session: {}, messages: [], tool_calls: [], events: [], approvals: [] };
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
    setAllSessions(sessions.sessions || []);
  }

  async function fetchActiveSessions() {
    try {
      const data = await fetchJson(settings.apiBase, "/api/sessions");
      const running = (data.sessions || []).filter((s) => s.status === "running");
      setActiveSessions(running);
      setAllSessions(data.sessions || []);
    } catch {
      // silently ignore
    }
  }

  async function fetchMemoryItems(query = "") {
    try {
      const qs = query ? `?query=${encodeURIComponent(query)}` : "";
      const data = await fetchJson(settings.apiBase, `/api/memory${qs}`);
      setMemoryItems(data.memories || []);
    } catch {
      setMemoryItems([]);
    }
  }

  async function fetchMcpTools(serverId) {
    try {
      const data = await fetchJson(settings.apiBase, `/api/mcp/servers/${encodeURIComponent(serverId)}/tools`);
      setMcpTools(data.tools || []);
    } catch {
      setMcpTools([]);
    }
  }

  useEffect(() => {
    refreshAll().catch((error) => setStatus(`Refresh failed: ${error.message}`));
  }, [settings.apiBase]);

  useEffect(() => {
    if (!settings.autoRefresh) return undefined;
    const timer = setInterval(() => {
      refreshAll().catch(() => {});
      if (activeSessions.length > 0) fetchActiveSessions();
    }, settings.refreshMs);
    return () => clearInterval(timer);
  }, [settings.autoRefresh, settings.refreshMs, settings.apiBase, activeSessions.length]);

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
        if (message.event.event_type === "SessionStarted") {
          fetchActiveSessions();
        }
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
    const handleKeyDown = (event) => {
      if (event.key === "Escape") {
        if (mcpToolsModal) { setMcpToolsModal(null); return; }
        if (selectedApproval) { setSelectedApprovalId(null); return; }
      }
      if (event.ctrlKey && event.key === "Enter") {
        const target = event.target;
        if (target.tagName === "TEXTAREA" && target.closest("form") === runFormRef.current) {
          event.preventDefault();
          runFormRef.current?.querySelector('button[type="submit"]')?.click();
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedApproval, mcpToolsModal]);

  function saveSettings(event) {
    event.preventDefault();
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    setStatus("Settings saved.");
    refreshAll().catch((error) => setStatus(`Refresh failed: ${error.message}`));
  }

  async function saveAgent(event) {
    event.preventDefault();
    const payload = {
      id: agentForm.id.trim(),
      name: agentForm.name.trim(),
      system_prompt: agentForm.prompt,
      tools: ["file_read", "file_list", "workspace_search"],
      model: {
        provider: agentForm.provider.trim(),
        model: agentForm.model.trim(),
        context_window: 8192,
        temperature: 0.2,
        top_p: null,
        max_tokens: null,
      },
      permissions: {
        file_read: "ask",
        file_list: "ask",
        workspace_search: "ask",
      },
      max_iterations: 12,
    };
    try {
      await fetchJson(settings.apiBase, "/api/agents", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setStatus(`Saved agent ${payload.id}`);
      setRunForm((current) => ({ ...current, agentId: payload.id }));
      await refreshAll();
    } catch (error) {
      setStatus(`Agent save failed: ${error.message}`);
    }
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
    try {
      const result = await fetchJson(settings.apiBase, "/api/sessions/run", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setLastRun({ sessionId: result.session_id || "", final: result.final || "" });
      setStatus(`Run completed for ${payload.agent_id} (${result.session_id || "no session id"}).`);
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
      await fetchActiveSessions();
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

  async function addMcpServer(event) {
    event.preventDefault();
    const argsArray = mcpForm.args.split(",").map((s) => s.trim()).filter(Boolean);
    const payload = {
      name: mcpForm.name.trim(),
      transport: "stdio",
      command: mcpForm.command.trim(),
      args: argsArray,
      env: {},
      enabled: mcpForm.enabled,
    };
    try {
      await fetchJson(settings.apiBase, "/api/mcp/servers", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setStatus(`MCP server ${payload.name} added.`);
      setMcpForm({ name: "", command: "", args: "", enabled: true });
      await refreshAll();
    } catch (error) {
      setStatus(`MCP add failed: ${error.message}`);
    }
  }

  async function toggleMcpServer(server) {
    try {
      await fetchJson(
        settings.apiBase,
        `/api/mcp/servers/${encodeURIComponent(server.id || server.name)}`,
        { method: "PATCH", headers: { "content-type": "application/json" }, body: JSON.stringify({ enabled: !server.enabled }) },
      );
      await refreshAll();
    } catch (error) {
      setStatus(`MCP update failed: ${error.message}`);
    }
  }

  async function deleteMcpServer(server) {
    try {
      await fetchJson(
        settings.apiBase,
        `/api/mcp/servers/${encodeURIComponent(server.id || server.name)}`,
        { method: "DELETE" },
      );
      setStatus(`MCP server ${server.name} deleted.`);
      await refreshAll();
    } catch (error) {
      setStatus(`MCP delete failed: ${error.message}`);
    }
  }

  async function browseMcpTools(server) {
    setMcpToolsModal(server);
    await fetchMcpTools(server.id || server.name);
  }

  async function writeMemory(event) {
    event.preventDefault();
    const payload = {
      content: memoryForm.content.trim(),
      scope: memoryForm.scope,
      tags: memoryForm.tags.split(",").map((s) => s.trim()).filter(Boolean),
      importance: memoryForm.importance,
    };
    try {
      await fetchJson(settings.apiBase, "/api/memory", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setStatus("Memory saved.");
      setMemoryForm({ content: "", scope: "agent", tags: "", importance: 0 });
      await fetchMemoryItems(memorySearch);
    } catch (error) {
      setStatus(`Memory write failed: ${error.message}`);
    }
  }

  async function deleteMemory(id) {
    try {
      await fetchJson(settings.apiBase, `/api/memory/${encodeURIComponent(id)}`, { method: "DELETE" });
      setStatus("Memory deleted.");
      await fetchMemoryItems(memorySearch);
    } catch (error) {
      setStatus(`Memory delete failed: ${error.message}`);
    }
  }

  async function viewSessionTrace(sessionId) {
    try {
      const data = await fetchJson(settings.apiBase, `/api/sessions/${encodeURIComponent(sessionId)}/trace`);
      setViewTraceSession(data);
      setActivePane("traces");
    } catch (error) {
      setStatus(`Failed to load trace: ${error.message}`);
    }
  }

  function renderAgentCards() {
    const agents = trace.agents || [];
    if (agents.length === 0) {
      return <p className="muted">No agents saved. Use the Agent Builder above to create one.</p>;
    }
    return agents.map((agent) => (
      <article key={agent.id} className="card">
        <header className="row">
          <strong>{agent.name}</strong>
          <span style={{ color: "var(--ink-4)", fontSize: "0.78rem" }}>
            {agent.model?.provider}/{agent.model?.model}
          </span>
        </header>
        <p style={{ margin: "4px 0", color: "var(--ink-3)", fontSize: "0.82rem" }}>
          {agent.description || "No description"}
        </p>
        <pre style={{ fontSize: "0.72rem", color: "var(--ink-4)", marginTop: 4 }}>
          ID: {agent.id}
        </pre>
        <button
          style={{ marginTop: 6 }}
          onClick={() => setRunForm((f) => ({ ...f, agentId: agent.id }))}
        >
          Use this agent
        </button>
      </article>
    ));
  }

  function renderMemoryItems() {
    if (memoryItems.length === 0) {
      return <p className="muted">No memory items yet. Use "New Memory" to store something.</p>;
    }
    return memoryItems.map((item) => (
      <article key={item.id} className="card">
        <header className="row">
          <strong>{item.scope}</strong>
          <span style={{ color: "var(--ink-4)", fontSize: "0.72rem" }}>
            {"★".repeat(Math.max(0, Math.min(5, item.importance || 0)))}
          </span>
        </header>
        <p style={{ margin: "4px 0", fontSize: "0.84rem" }}>{item.content}</p>
        {item.tags?.length > 0 && (
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
            {item.tags.map((tag) => (
              <span key={tag} className="approval-chip">{tag}</span>
            ))}
          </div>
        )}
        <div className="row" style={{ marginTop: 6 }}>
          <button className="ghost" style={{ fontSize: "0.75rem", padding: "4px 8px" }} onClick={() => deleteMemory(item.id)}>
            Delete
          </button>
        </div>
      </article>
    ));
  }

  function renderActiveSessions() {
    if (activeSessions.length === 0) {
      return <p className="muted">No active sessions.</p>;
    }
    return activeSessions.map((session) => (
      <article key={session.id} className="card" style={{ borderColor: "rgba(44, 224, 163, 0.3)" }}>
        <header className="row">
          <span className="pulsing-dot" />
          <strong style={{ fontFamily: "var(--mono)", fontSize: "0.78rem" }}>
            {session.id.slice(0, 12)}...
          </strong>
          <span style={{ color: "var(--accent-a)", fontSize: "0.75rem" }}>running</span>
        </header>
        <p style={{ margin: "3px 0", fontSize: "0.8rem", color: "var(--ink-3)" }}>
          {session.title || "untitled"} · {session.root_agent_id}
        </p>
        <button
          className="danger"
          style={{ marginTop: 4, fontSize: "0.78rem" }}
          onClick={() => cancelSession(session.id)}
        >
          Cancel
        </button>
      </article>
    ));
  }

  function renderAllSessions() {
    if (allSessions.length === 0) {
      return <p className="muted">No sessions recorded yet.</p>;
    }
    return allSessions.map((session) => {
      const statusColors = {
        running: "var(--accent-a)",
        completed: "var(--accent-b)",
        failed: "var(--danger)",
        cancelled: "#ffb347",
      };
      return (
        <article key={session.id} className="event-card">
          <header className="row">
            <strong style={{ color: statusColors[session.status] || "var(--ink-3)" }}>
              {session.status}
            </strong>
            <span style={{ fontFamily: "var(--mono)", fontSize: "0.72rem" }}>
              {session.root_agent_id}
            </span>
          </header>
          <p style={{ margin: "2px 0", fontSize: "0.8rem" }}>{session.title || "untitled"}</p>
          <div className="row" style={{ marginTop: 4 }}>
            <button
              className="ghost"
              style={{ fontSize: "0.75rem", padding: "3px 8px" }}
              onClick={() => viewSessionTrace(session.id)}
            >
              View trace
            </button>
            {session.status === "running" && (
              <button
                className="danger"
                style={{ fontSize: "0.75rem", padding: "3px 8px" }}
                onClick={() => cancelSession(session.id)}
              >
                Cancel
              </button>
            )}
          </div>
        </article>
      );
    });
  }

  return (
    <div className="deck">
      <div className="bg-mesh" />

      <header className="mast">
        <div>
          <p className="micro">Agent1 Desktop</p>
          <h1>Command Atlas</h1>
          <p className="subtitle">
            Real-time agent orchestration with full event telemetry and approval control.
          </p>
        </div>
        <div className="mast-actions">
          {isRunning && <span className="streaming-pill">Streaming...</span>}
          <span className={`ws-pill ws-${wsState}`}>WS {wsState}</span>
          <button onClick={() => refreshAll().catch((error) => setStatus(error.message))}>
            Sync now
          </button>
        </div>
      </header>

      <main className="layout">
        <aside className="rail">
          <section className="panel">
            <h2>Control Surface</h2>
            <form onSubmit={saveSettings} className="stack">
              <label>
                API Base
                <input
                  value={settings.apiBase}
                  onChange={(event) =>
                    setSettings((current) => ({ ...current, apiBase: event.target.value }))
                  }
                />
              </label>
              <div className="split">
                <label>
                  Refresh ms
                  <input
                    type="number"
                    min={500}
                    value={settings.refreshMs}
                    onChange={(event) =>
                      setSettings((current) => ({
                        ...current,
                        refreshMs: Number(event.target.value || 2000),
                      }))
                    }
                  />
                </label>
                <label className="inline">
                  <input
                    type="checkbox"
                    checked={settings.autoRefresh}
                    onChange={(event) =>
                      setSettings((current) => ({ ...current, autoRefresh: event.target.checked }))
                    }
                  />
                  Auto-refresh
                </label>
              </div>
              <button type="submit">Save Settings</button>
            </form>
          </section>

          <section className="panel">
            <h2>Active Sessions</h2>
            {renderActiveSessions()}
          </section>

          <section className="panel">
            <h2>Run Agent</h2>
            <form ref={runFormRef} onSubmit={runAgent} className="stack">
              <label>
                Agent
                <input
                  list="agent-ids"
                  value={runForm.agentId}
                  onChange={(event) =>
                    setRunForm((current) => ({ ...current, agentId: event.target.value }))
                  }
                />
              </label>
              <datalist id="agent-ids">
                {(trace.agents || []).map((agent) => (
                  <option key={agent.id} value={agent.id} />
                ))}
              </datalist>
              <label>
                Task Input
                <textarea
                  rows={4}
                  value={runForm.input}
                  onChange={(event) =>
                    setRunForm((current) => ({ ...current, input: event.target.value }))
                  }
                />
              </label>
              <label>
                Workspace
                <input
                  value={runForm.workspace}
                  onChange={(event) =>
                    setRunForm((current) => ({ ...current, workspace: event.target.value }))
                  }
                />
              </label>
              <button type="submit" disabled={isRunning}>
                {isRunning ? "Running..." : "Run Session"}
              </button>
            </form>
            {lastRun && (
              <article className="card run-result">
                <header className="row">
                  <strong>Last Run</strong>
                  <span>{lastRun.sessionId || "session unavailable"}</span>
                </header>
                <pre>{lastRun.final || "No final answer returned."}</pre>
              </article>
            )}
          </section>

          <section className="panel">
            <h2>Agent Builder</h2>
            <form onSubmit={saveAgent} className="stack">
              <label>
                Agent ID
                <input
                  value={agentForm.id}
                  onChange={(event) =>
                    setAgentForm((current) => ({ ...current, id: event.target.value }))
                  }
                />
              </label>
              <label>
                Name
                <input
                  value={agentForm.name}
                  onChange={(event) =>
                    setAgentForm((current) => ({ ...current, name: event.target.value }))
                  }
                />
              </label>
              <div className="split">
                <label>
                  Provider
                  <select
                    value={agentForm.provider}
                    onChange={(event) =>
                      setAgentForm((current) => ({ ...current, provider: event.target.value }))
                    }
                    style={{
                      border: "1px solid rgba(255,255,255,0.16)",
                      borderRadius: 10,
                      padding: "8px 9px",
                      color: "var(--ink-1)",
                      background: "rgba(8,12,18,0.52)",
                    }}
                  >
                    <option value="ollama">ollama</option>
                    <option value="openai_compatible">openai-compatible</option>
                    <option value="mock">mock</option>
                  </select>
                </label>
                <label>
                  Model
                  <input
                    value={agentForm.model}
                    onChange={(event) =>
                      setAgentForm((current) => ({ ...current, model: event.target.value }))
                    }
                  />
                </label>
              </div>
              <label>
                System Prompt
                <textarea
                  rows={4}
                  value={agentForm.prompt}
                  onChange={(event) =>
                    setAgentForm((current) => ({ ...current, prompt: event.target.value }))
                  }
                />
              </label>
              <button type="submit">Save Agent</button>
            </form>
          </section>

          <section className="panel">
            <h2>Add MCP Server</h2>
            <form onSubmit={addMcpServer} className="stack">
              <label>
                Name
                <input
                  value={mcpForm.name}
                  onChange={(event) =>
                    setMcpForm((f) => ({ ...f, name: event.target.value }))
                  }
                  placeholder="filesystem"
                  required
                />
              </label>
              <label>
                Command
                <input
                  value={mcpForm.command}
                  onChange={(event) =>
                    setMcpForm((f) => ({ ...f, command: event.target.value }))
                  }
                  placeholder="npx"
                  required
                />
              </label>
              <label>
                Args (comma-separated)
                <input
                  value={mcpForm.args}
                  onChange={(event) =>
                    setMcpForm((f) => ({ ...f, args: event.target.value }))
                  }
                  placeholder="@modelcontextprotocol/server-filesystem, ."
                />
              </label>
              <label className="inline">
                <input
                  type="checkbox"
                  checked={mcpForm.enabled}
                  onChange={(event) =>
                    setMcpForm((f) => ({ ...f, enabled: event.target.checked }))
                  }
                />
                Enabled
              </label>
              <button type="submit">Add MCP Server</button>
            </form>
          </section>
        </aside>

        <section className="surface">
          <section className="metrics">
            <article>
              <p>Session</p>
              <strong>{safeText(trace.session?.id, "No active session")}</strong>
            </article>
            <article>
              <p>Messages</p>
              <strong>{sessionMetrics.messages}</strong>
            </article>
            <article>
              <p>Tool Calls</p>
              <strong>{sessionMetrics.tools}</strong>
            </article>
            <article>
              <p>Events</p>
              <strong>{sessionMetrics.events}</strong>
            </article>
            <article>
              <p>Errors</p>
              <strong>{sessionMetrics.errors}</strong>
            </article>
          </section>

          <section className="panel">
            <div className="panel-header">
              <h2>Pending Approvals</h2>
              <span>{pendingApprovals.length}</span>
            </div>
            {pendingApprovals.length === 0 ? (
              <p className="muted">No pending approval gates.</p>
            ) : (
              <div className="approval-grid">
                {pendingApprovals.map((approval) => (
                  <article key={approval.id} className="approval-card">
                    <strong>{approval.id}</strong>
                    <pre>{JSON.stringify(approval.request || {}, null, 2)}</pre>
                    <div className="row">
                      <button onClick={() => setSelectedApprovalId(approval.id)}>Review</button>
                      <button onClick={() => decideApproval(approval.id, "approved")}>Approve</button>
                      <button className="danger" onClick={() => decideApproval(approval.id, "denied")}>
                        Deny
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>

          <section className="split-panels">
            <section className="panel">
              <div className="panel-header">
                <h2>Model Providers</h2>
              </div>
              <div className="stack compact">
                {(trace.model_providers || []).length === 0 && <p className="muted">No providers loaded.</p>}
                {(trace.model_providers || []).map((provider, index) => (
                  <article key={`${provider.provider}-${index}`} className="card">
                    <header className="row">
                      <strong>{provider.provider}</strong>
                      <span>{provider.error ? "error" : "ok"}</span>
                    </header>
                    <pre>{JSON.stringify(provider.error || provider.models || [], null, 2)}</pre>
                  </article>
                ))}
              </div>
            </section>

            <section className="panel">
              <div className="panel-header">
                <h2>MCP Servers</h2>
              </div>
              <div className="stack compact">
                {(trace.mcp_servers || []).length === 0 && (
                  <p className="muted">No MCP servers configured. Add one using the form above.</p>
                )}
                {(trace.mcp_servers || []).map((server) => (
                  <article key={server.id || server.name} className="card">
                    <header className="row">
                      <strong>{server.name || server.id}</strong>
                      <span
                        style={{
                          color: server.enabled ? "var(--accent-a)" : "var(--ink-4)",
                          fontSize: "0.78rem",
                        }}
                      >
                        {server.enabled ? "enabled" : "disabled"}
                      </span>
                    </header>
                    <pre style={{ fontSize: "0.72rem" }}>
                      {JSON.stringify({ command: server.command, args: server.args }, null, 2)}
                    </pre>
                    <div className="row" style={{ marginTop: 4 }}>
                      <button
                        className="ghost"
                        style={{ fontSize: "0.75rem", padding: "3px 8px" }}
                        onClick={() => browseMcpTools(server)}
                      >
                        Tools
                      </button>
                      <button
                        className="ghost"
                        style={{ fontSize: "0.75rem", padding: "3px 8px" }}
                        onClick={() => toggleMcpServer(server)}
                      >
                        {server.enabled ? "Disable" : "Enable"}
                      </button>
                      <button
                        className="danger"
                        style={{ fontSize: "0.75rem", padding: "3px 8px" }}
                        onClick={() => deleteMcpServer(server)}
                      >
                        Delete
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            </section>
          </section>

          <section className="panel">
            <div className="panel-header">
              <h2>Session Explorer</h2>
              <span>{trace.session?.id || "none"}</span>
            </div>
            <div className="tabs">
              <button className={activePane === "events" ? "tab active" : "tab"} onClick={() => setActivePane("events")}>
                Events
              </button>
              <button className={activePane === "messages" ? "tab active" : "tab"} onClick={() => setActivePane("messages")}>
                Messages
              </button>
              <button className={activePane === "tools" ? "tab active" : "tab"} onClick={() => setActivePane("tools")}>
                Tools
              </button>
              <button className={activePane === "agents" ? "tab active" : "tab"} onClick={() => setActivePane("agents")}>
                Agents
              </button>
              <button className={activePane === "sessions" ? "tab active" : "tab"} onClick={() => setActivePane("sessions")}>
                Sessions
              </button>
              <button className={activePane === "memory" ? "tab active" : "tab"} onClick={() => { setActivePane("memory"); fetchMemoryItems(); }}>
                Memory
              </button>
            </div>

            {activePane === "events" && (
              <div className="stack compact">
                {recentEvents.length === 0 && <p className="muted">No events recorded for this session.</p>}
                {recentEvents.map((event) => (
                  <article key={event.id || `${event.created_at}-${event.event_type}`} className="event-card">
                    <header className="row">
                      <strong>{event.event_type}</strong>
                      <span>{event.created_at}</span>
                    </header>
                    <pre>{JSON.stringify(event.payload || {}, null, 2)}</pre>
                  </article>
                ))}
              </div>
            )}

            {activePane === "messages" && (
              <div className="stack compact">
                {recentMessages.length === 0 && <p className="muted">No messages yet. Run an agent to start a conversation.</p>}
                {recentMessages.map((message) => (
                  <article key={message.id || `${message.created_at}-${message.role}`} className="event-card">
                    <header className="row">
                      <strong>{message.role}</strong>
                      <span>{message.from_agent_id || "user"}</span>
                    </header>
                    <pre>{message.content || ""}</pre>
                  </article>
                ))}
              </div>
            )}

            {activePane === "tools" && (
              <div className="stack compact">
                {recentToolCalls.length === 0 && <p className="muted">No tool calls yet.</p>}
                {recentToolCalls.map((call) => (
                  <article key={call.id} className="event-card">
                    <header className="row">
                      <strong>{call.tool_name}</strong>
                      <span>{call.status}</span>
                    </header>
                    <pre>
                      {JSON.stringify({ input: call.input, output: call.output, error: call.error }, null, 2)}
                    </pre>
                  </article>
                ))}
              </div>
            )}

            {activePane === "agents" && (
              <div className="stack compact">{renderAgentCards()}</div>
            )}

            {activePane === "sessions" && (
              <div className="stack compact">{renderAllSessions()}</div>
            )}

            {activePane === "memory" && (
              <div className="stack compact">
                <div style={{ display: "flex", gap: 6, marginBottom: 8 }}>
                  <input
                    type="text"
                    placeholder="Search memories..."
                    value={memorySearch}
                    onChange={(e) => setMemorySearch(e.target.value)}
                    onKeyDown={(e) => { if (e.key === "Enter") fetchMemoryItems(memorySearch); }}
                    style={{ flex: 1 }}
                  />
                  <button onClick={() => fetchMemoryItems(memorySearch)}>Search</button>
                </div>
                {renderMemoryItems()}
                <hr style={{ border: "none", borderTop: "1px solid rgba(255,255,255,0.08)", margin: "12px 0" }} />
                <h3 style={{ margin: "0 0 8px", fontSize: "0.85rem", color: "var(--ink-3)" }}>New Memory</h3>
                <form onSubmit={writeMemory} className="stack">
                  <label>
                    Content
                    <textarea
                      rows={3}
                      value={memoryForm.content}
                      onChange={(e) => setMemoryForm((f) => ({ ...f, content: e.target.value }))}
                      placeholder="Remember that..."
                      required
                    />
                  </label>
                  <div className="split">
                    <label>
                      Scope
                      <select
                        value={memoryForm.scope}
                        onChange={(e) => setMemoryForm((f) => ({ ...f, scope: e.target.value }))}
                        style={{
                          border: "1px solid rgba(255,255,255,0.16)",
                          borderRadius: 10,
                          padding: "8px 9px",
                          color: "var(--ink-1)",
                          background: "rgba(8,12,18,0.52)",
                        }}
                      >
                        <option value="agent">agent</option>
                        <option value="global">global</option>
                      </select>
                    </label>
                    <label>
                      Tags (comma-sep)
                      <input
                        value={memoryForm.tags}
                        onChange={(e) => setMemoryForm((f) => ({ ...f, tags: e.target.value }))}
                        placeholder="preference, project"
                      />
                    </label>
                  </div>
                  <label>
                    Importance (0–5): {memoryForm.importance}
                    <input
                      type="range"
                      min={0}
                      max={5}
                      value={memoryForm.importance}
                      onChange={(e) =>
                        setMemoryForm((f) => ({ ...f, importance: Number(e.target.value) }))
                      }
                    />
                  </label>
                  <button type="submit">Save Memory</button>
                </form>
              </div>
            )}
          </section>
        </section>
      </main>

      {selectedApproval && (
        <div className="modal-wrap" onClick={() => setSelectedApprovalId(null)}>
          <div className="modal-card" onClick={(event) => event.stopPropagation()}>
            <header className="row">
              <h3>Approval Required</h3>
              <button className="ghost" onClick={() => setSelectedApprovalId(null)}>
                Close
              </button>
            </header>
            <p className="muted">
              {selectedApproval.agent_id} requests{" "}
              {selectedApproval.request?.tool_name || selectedApproval.request?.tool || "tool"}
            </p>
            <pre>{JSON.stringify(selectedApproval.request || {}, null, 2)}</pre>
            <div className="row">
              <button onClick={() => decideApproval(selectedApproval.id, "approved")}>Approve</button>
              <button className="danger" onClick={() => decideApproval(selectedApproval.id, "denied")}>
                Deny
              </button>
            </div>
          </div>
        </div>
      )}

      {mcpToolsModal && (
        <div className="modal-wrap" onClick={() => setMcpToolsModal(null)}>
          <div className="modal-card" onClick={(event) => event.stopPropagation()}>
            <header className="row">
              <h3>MCP Tools: {mcpToolsModal.name}</h3>
              <button className="ghost" onClick={() => setMcpToolsModal(null)}>Close</button>
            </header>
            <div className="stack compact">
              {mcpTools.length === 0 && <p className="muted">No tools found or server unavailable.</p>}
              {mcpTools.map((tool, i) => (
                <article key={i} className="card">
                  <strong>{tool.name}</strong>
                  <p style={{ margin: "2px 0", fontSize: "0.8rem", color: "var(--ink-3)" }}>
                    {tool.description}
                  </p>
                  <pre style={{ fontSize: "0.72rem" }}>
                    {JSON.stringify(tool.inputSchema || tool.input_schema || {}, null, 2)}
                  </pre>
                </article>
              ))}
            </div>
          </div>
        </div>
      )}

      <footer className="dock">
        <span>{status || "System ready"}</span>
        <span>
          {trace.session?.status ? `Session: ${trace.session.status}` : "Session idle"}
          {" · "}
          {wsState === "connected" ? "WS connected" : "WS disconnected"}
        </span>
      </footer>
    </div>
  );
}