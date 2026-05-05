import { useEffect, useMemo, useRef, useState } from "react";
import { CanvasGraph } from "./components";

const SETTINGS_KEY = "agent1_desktop_settings_v1";
const AGENT1_ID = "agent1";
const AGENT1_SYSTEM_PROMPT =
  "You are Agent1, the central orchestrator for this desktop app. Talk directly with the user, coordinate local worker agents, use local files and tools when approved, browse through connected tools, create tools when needed, and manage supporting agents. Keep responses practical, clear, and action-oriented.";
const AGENT1_TOOLS = [
  "file_read",
  "file_list",
  "workspace_search",
  "git_status",
  "git_diff",
  "task_board",
  "memory_search",
  "memory_write",
  "agent_call",
  "mcp_call",
];
const AGENT1_PERMISSIONS = {
  file_read: "ask",
  file_list: "ask",
  workspace_search: "ask",
  git_status: "ask",
  git_diff: "ask",
  task_board: "ask",
  memory_search: "ask",
  memory_write: "ask",
  agent_call: "ask",
  mcp_call: "ask",
  file_write: "deny",
  shell: "deny",
};

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

function isAgent1Agent(agent) {
  const id = agent?.id?.toLowerCase?.() || "";
  const name = agent?.name?.toLowerCase?.() || "";
  const role = agent?.role?.toLowerCase?.() || "";
  return id === AGENT1_ID || name === "agent1" || role.includes("orchestrator");
}

function buildAgent1Payload(form, existingAgent) {
  return {
    ...(existingAgent || {}),
    id: existingAgent?.id || AGENT1_ID,
    name: "Agent1",
    description: "Central user-facing orchestrator and controller",
    role: "Orchestrator",
    system_prompt: AGENT1_SYSTEM_PROMPT,
    tools: AGENT1_TOOLS,
    model: {
      provider: form.provider.trim(),
      model: form.model.trim(),
      base_url: existingAgent?.model?.base_url || undefined,
      context_window: existingAgent?.model?.context_window || 8192,
      temperature: existingAgent?.model?.temperature ?? 0.2,
    },
    memory: { ...(existingAgent?.memory || {}), enabled: true },
    permissions: { ...(existingAgent?.permissions || {}), ...AGENT1_PERMISSIONS },
    max_iterations: existingAgent?.max_iterations || 16,
  };
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
    input: "",
    workspace: ".",
  });
  const [conversation, setConversation] = useState([]);
  const [lastRun, setLastRun] = useState(null);
  const [activeSessions, setActiveSessions] = useState([]);
  const [status, setStatus] = useState("");
  const [wsState, setWsState] = useState("disconnected");
  const [selectedAgentId, setSelectedAgentId] = useState(null);
  const [isRunning, setIsRunning] = useState(false);
  const [agentTab, setAgentTab] = useState("all");
  const [selectedApprovalId, setSelectedApprovalId] = useState(null);
  const [expandedEvents, setExpandedEvents] = useState(new Set());
  const [agentBuilderOpen, setAgentBuilderOpen] = useState(false);
  const [agent1ConfigOpen, setAgent1ConfigOpen] = useState(false);
  const [agent1FormStatus, setAgent1FormStatus] = useState("");
  const [agent1Form, setAgent1Form] = useState({
    provider: "opencode",
    model: "",
  });
  const [agentForm, setAgentForm] = useState({
    id: "",
    name: "",
    provider: "opencode",
    model: "",
    systemPrompt: "You are a practical local agent. Be concise and direct.",
  });
  const [agentFormStatus, setAgentFormStatus] = useState("");
  const [externalBuilderOpen, setExternalBuilderOpen] = useState(false);
  const [externalFormStatus, setExternalFormStatus] = useState("");
  const [externalForm, setExternalForm] = useState({
    name: "",
    command: "",
    args: "",
    enabled: true,
  });
  const [whatsappOpen, setWhatsappOpen] = useState(false);
  const [whatsappStatus, setWhatsappStatus] = useState({ state: "disconnected", phone: null });
  const [whatsappQr, setWhatsappQr] = useState(null);
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
  const agent1Agent = useMemo(() => {
    const agents = trace.agents || [];
    return (
      agents.find((agent) => agent.id?.toLowerCase?.() === AGENT1_ID) ||
      agents.find((agent) => agent.name?.toLowerCase?.() === "agent1") ||
      agents.find((agent) => agent.role?.toLowerCase?.().includes("orchestrator")) ||
      agents.find((agent) => agent.id === "assistant") ||
      null
    );
  }, [trace.agents]);
  const supportAgents = useMemo(
    () => (trace.agents || []).filter((agent) => !isAgent1Agent(agent) && agent.id !== agent1Agent?.id),
    [trace.agents, agent1Agent]
  );

  const filteredAgents = useMemo(() => {
    const agents = supportAgents;
    if (agentTab === "all") return agents;
    const runningIds = new Set(activeSessions.map((s) => s.root_agent_id));
    if (agentTab === "running") return agents.filter((a) => runningIds.has(a.id));
    if (agentTab === "idle") return agents.filter((a) => !runningIds.has(a.id));
    return agents;
  }, [supportAgents, activeSessions, agentTab]);

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

  async function ensureAgent1Agent() {
    if (agent1Agent) return agent1Agent.id;
    const payload = buildAgent1Payload(agent1Form, null);
    if (!payload.model.provider || !payload.model.model) {
      throw new Error("Configure Agent1 provider and model before sending a command.");
    }
    const result = await fetchJson(settings.apiBase, "/api/agents", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    await refreshAll();
    return result.agent?.id || payload.id;
  }

  async function runAgent(event) {
    event.preventDefault();
    const input = runForm.input.trim();
    if (!input) {
      setStatus("Run blocked: command input is required.");
      return;
    }
    const workspace = runForm.workspace.trim() || ".";
    setConversation((items) => [
      ...items,
      { id: `user-${Date.now()}`, role: "user", content: input },
    ]);
    setRunForm((form) => ({ ...form, input: "" }));
    setIsRunning(true);
    try {
      const agentId = await ensureAgent1Agent();
      const payload = {
        agent_id: agentId,
        input,
        workspace,
      };
      setSelectedAgentId(agentId);
      const result = await fetchJson(settings.apiBase, "/api/sessions/run", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setLastRun({ sessionId: result.session_id || "", final: result.final || "" });
      setConversation((items) => [
        ...items,
        {
          id: `agent1-${result.session_id || Date.now()}`,
          role: "agent1",
          content: result.final || "Done.",
        },
      ]);
      setStatus("Agent1 response complete.");
      await refreshAll();
    } catch (error) {
      setStatus(`Run failed: ${error.message}`);
      setConversation((items) => [
        ...items,
        { id: `error-${Date.now()}`, role: "agent1 error", content: error.message },
      ]);
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

  async function refreshWhatsAppStatus() {
    try {
      const status = await fetchJson(settings.apiBase, "/api/whatsapp/status");
      setWhatsappStatus(status);
      if (status.qr) {
        setWhatsappQr(status.qr);
      }
    } catch (error) {
      setWhatsappStatus({ state: "sidecar_offline", phone: null });
    }
  }

  async function connectWhatsApp() {
    try {
      setStatus("Connecting to WhatsApp...");
      await fetchJson(settings.apiBase, "/api/whatsapp/connect", {
        method: "POST",
      });
      setWhatsappStatus({ state: "connecting", phone: null });
      setStatus("WhatsApp connecting...");
      const poll = async () => {
        await refreshWhatsAppStatus();
        if (whatsappStatus.state === "connecting" || whatsappStatus.state === "qr_ready") {
          setTimeout(poll, 2000);
        }
      };
      poll();
    } catch (error) {
      setStatus(`WhatsApp connect failed: ${error.message}`);
    }
  }

  async function disconnectWhatsApp() {
    try {
      await fetchJson(settings.apiBase, "/api/whatsapp/disconnect", {
        method: "POST",
      });
      setWhatsappStatus({ state: "disconnected", phone: null });
      setWhatsappQr(null);
      setStatus("WhatsApp disconnected");
    } catch (error) {
      setStatus(`WhatsApp disconnect failed: ${error.message}`);
    }
  }

  async function resetWhatsApp() {
    try {
      await fetchJson(settings.apiBase, "/api/whatsapp/reset", {
        method: "POST",
      });
      setWhatsappStatus({ state: "disconnected", phone: null, qr: null, error: null });
      setWhatsappQr(null);
      setStatus("WhatsApp session reset");
    } catch (error) {
      setStatus(`WhatsApp reset failed: ${error.message}`);
    }
  }

  async function fetchWhatsAppQr() {
    try {
      const resp = await fetch(`${settings.apiBase}/api/whatsapp/qr`);
      if (resp.headers.get("content-type")?.includes("image/svg")) {
        const svg = await resp.text();
        setWhatsappQr(`data:image/svg+xml;base64,${btoa(svg)}`);
      } else {
        const data = await resp.json();
        if (data.qr) {
          setWhatsappQr(data.qr);
        }
      }
    } catch (error) {
      console.error("Failed to fetch QR:", error);
    }
  }

  async function createAgent(event) {
    event.preventDefault();
    const payload = {
      id: agentForm.id.trim(),
      name: agentForm.name.trim(),
      system_prompt: agentForm.systemPrompt.trim(),
      tools: ["file_read", "file_list", "workspace_search"],
      model: {
        provider: agentForm.provider.trim(),
        model: agentForm.model.trim(),
        context_window: 8192,
        temperature: 0.2,
      },
      permissions: {
        file_read: "ask",
        file_list: "ask",
        workspace_search: "ask",
      },
      max_iterations: 12,
    };
    if (!payload.id || !payload.name || !payload.model.provider || !payload.model.model) {
      setAgentFormStatus("ID, name, and model are required.");
      return;
    }
    setAgentFormStatus("Saving...");
    try {
      const result = await fetchJson(settings.apiBase, "/api/agents", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setAgentFormStatus(`Saved agent ${result.agent?.id || payload.id}`);
      setAgentForm({ id: "", name: "", provider: "opencode", model: defaultOpenCodeModel, systemPrompt: "You are a practical local agent. Be concise and direct." });
      setAgentBuilderOpen(false);
      await refreshAll();
    } catch (error) {
      setAgentFormStatus(`Save failed: ${error.message}`);
    }
  }

  async function saveAgent1Config(event) {
    event?.preventDefault();
    const payload = buildAgent1Payload(agent1Form, agent1Agent);
    if (!payload.model.provider || !payload.model.model) {
      setAgent1FormStatus("Provider and model are required.");
      return;
    }
    setAgent1FormStatus("Saving...");
    try {
      const result = await fetchJson(settings.apiBase, "/api/agents", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setAgent1FormStatus(`Saved ${result.agent?.name || "Agent1"}`);
      setSelectedAgentId(result.agent?.id || payload.id);
      await refreshAll();
    } catch (error) {
      setAgent1FormStatus(`Save failed: ${error.message}`);
    }
  }

  async function createExternalAgent(event) {
    event.preventDefault();
    const name = externalForm.name.trim();
    const command = externalForm.command.trim();
    if (!name || !command) {
      setExternalFormStatus("Name and command are required.");
      return;
    }
    setExternalFormStatus("Inviting...");
    try {
      const payload = {
        name,
        transport: "stdio",
        command,
        args: externalForm.args
          .split(/\s+/)
          .map((part) => part.trim())
          .filter(Boolean),
        enabled: externalForm.enabled,
      };
      const result = await fetchJson(settings.apiBase, "/api/mcp/servers", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      setExternalFormStatus(`Invited ${result.server?.name || name}`);
      setExternalForm({ name: "", command: "", args: "", enabled: true });
      setExternalBuilderOpen(false);
      await refreshAll();
    } catch (error) {
      setExternalFormStatus(`Invite failed: ${error.message}`);
    }
  }

  async function updateExternalAgent(server, enabled) {
    try {
      await fetchJson(settings.apiBase, `/api/mcp/servers/${encodeURIComponent(server.id || server.name)}`, {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ enabled }),
      });
      setStatus(`${enabled ? "Resumed" : "Paused"} external agent ${server.name}`);
      await refreshAll();
    } catch (error) {
      setStatus(`External update failed: ${error.message}`);
    }
  }

  async function removeExternalAgent(server) {
    try {
      await fetchJson(settings.apiBase, `/api/mcp/servers/${encodeURIComponent(server.id || server.name)}`, {
        method: "DELETE",
      });
      setStatus(`Removed external agent ${server.name}`);
      await refreshAll();
    } catch (error) {
      setStatus(`External removal failed: ${error.message}`);
    }
  }

  function toggleEventExpand(eventId) {
    setExpandedEvents((prev) => {
      const next = new Set(prev);
      if (next.has(eventId)) next.delete(eventId);
      else next.add(eventId);
      return next;
    });
  }

  const runningAgentIds = useMemo(() => new Set(activeSessions.map((s) => s.root_agent_id)), [activeSessions]);
  const localAgentCount = supportAgents.length;
  const externalServerCount = trace.mcp_servers?.length || 0;
  const connectedExternalCount = (trace.mcp_servers || []).filter((server) => server.enabled).length;
  const wsLabel = wsState === "connected" ? "Connected" : wsState === "connecting" ? "Connecting" : wsState === "error" ? "Error" : "Offline";
  const providerOptions = useMemo(() => {
    const labels = {
      opencode: "OpenCode",
      ollama: "Ollama",
      openai_compatible: "OpenAI-compatible",
    };
    const fallback = ["opencode", "ollama", "openai_compatible"].map((provider) => ({
      provider,
      label: labels[provider] || provider,
      models: [],
      error: "",
    }));
    const fromServer = (trace.model_providers || []).map((item) => ({
      provider: item.provider,
      label: labels[item.provider] || item.provider,
      models: item.models || [],
      error: item.error || "",
    }));
    const merged = new Map(fallback.map((item) => [item.provider, item]));
    for (const item of fromServer) {
      merged.set(item.provider, { ...merged.get(item.provider), ...item });
    }
    return [...merged.values()];
  }, [trace.model_providers]);
  const selectedProvider = providerOptions.find((item) => item.provider === agentForm.provider) || providerOptions[0];
  const modelOptions = selectedProvider?.models || [];
  const selectedAgent1Provider = providerOptions.find((item) => item.provider === agent1Form.provider) || providerOptions[0];
  const agent1ModelOptions = selectedAgent1Provider?.models || [];
  const defaultOpenCodeModel =
    providerOptions.find((item) => item.provider === "opencode")?.models?.[0]?.name || "";
  const modelProviderError = selectedProvider?.error || "";
  const agent1ModelProviderError = selectedAgent1Provider?.error || "";

  useEffect(() => {
    if (!modelOptions.length) return;
    if (modelOptions.some((model) => model.name === agentForm.model)) return;
    setAgentForm((form) => ({ ...form, model: modelOptions[0].name }));
  }, [agentForm.model, modelOptions]);

  useEffect(() => {
    if (agent1Agent?.model) {
      setAgent1Form({
        provider: agent1Agent.model.provider || "opencode",
        model: agent1Agent.model.model || "",
      });
      return;
    }
    if (!agent1ModelOptions.length) return;
    if (agent1ModelOptions.some((model) => model.name === agent1Form.model)) return;
    setAgent1Form((form) => ({ ...form, model: agent1ModelOptions[0].name }));
  }, [agent1Agent, agent1Form.model, agent1ModelOptions]);

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
              <div className="empty-card compact">
                <span className="empty-icon">AI</span>
                <strong>No local agents configured yet.</strong>
                <span>Create your first agent to begin orchestration.</span>
              </div>
            ) : (
              filteredAgents.map((agent) => (
                <div
                  key={agent.id}
                  className={`agent-item ${selectedAgentId === agent.id ? "selected" : ""}`}
                  onClick={() => {
                    setSelectedAgentId(agent.id);
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

        <div className="section">
          <div className="section-title">Agent Builder</div>
          {!agentBuilderOpen ? (
            <button
              className="btn btn-primary"
              style={{ width: "100%", marginBottom: "8px" }}
              onClick={() => setAgentBuilderOpen(true)}
            >
              + New Agent
            </button>
          ) : (
            <form onSubmit={createAgent} style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
              <div className="form-group">
                <label>Agent ID</label>
                <input
                  type="text"
                  placeholder="my_agent"
                  value={agentForm.id}
                  onChange={(e) => setAgentForm((f) => ({ ...f, id: e.target.value }))}
                  style={{
                    width: "100%",
                    padding: "8px 10px",
                    background: "var(--surface-2)",
                    border: "1px solid var(--border-default)",
                    borderRadius: "var(--radius-sm)",
                    color: "var(--text-primary)",
                    fontFamily: "var(--font-mono)",
                    fontSize: "12px",
                  }}
                />
              </div>
              <div className="form-group">
                <label>Name</label>
                <input
                  type="text"
                  placeholder="My Agent"
                  value={agentForm.name}
                  onChange={(e) => setAgentForm((f) => ({ ...f, name: e.target.value }))}
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
                />
              </div>
              <div className="form-group">
                <label>Provider</label>
                <select
                  value={agentForm.provider}
                  onChange={(e) => {
                    const provider = e.target.value;
                    const nextProvider = providerOptions.find((item) => item.provider === provider);
                    setAgentForm((f) => ({
                      ...f,
                      provider,
                      model: nextProvider?.models?.[0]?.name || "",
                    }));
                  }}
                >
                  {providerOptions.map((provider) => (
                    <option key={provider.provider} value={provider.provider}>
                      {provider.label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-group">
                <label>Model</label>
                <select
                  value={agentForm.model}
                  onChange={(e) => setAgentForm((f) => ({ ...f, model: e.target.value }))}
                  disabled={!modelOptions.length}
                >
                  {!modelOptions.length && <option value="">No models loaded</option>}
                  {modelOptions.map((model) => (
                    <option key={`${selectedProvider?.provider}-${model.name}`} value={model.name}>
                      {model.name}
                    </option>
                  ))}
                </select>
                {modelProviderError && (
                  <div className="field-hint error">{modelProviderError}</div>
                )}
              </div>
              <div className="form-group">
                <label>System Prompt</label>
                <textarea
                  rows={3}
                  placeholder="You are..."
                  value={agentForm.systemPrompt}
                  onChange={(e) => setAgentForm((f) => ({ ...f, systemPrompt: e.target.value }))}
                  style={{
                    width: "100%",
                    padding: "8px 10px",
                    background: "var(--surface-2)",
                    border: "1px solid var(--border-default)",
                    borderRadius: "var(--radius-sm)",
                    color: "var(--text-primary)",
                    fontFamily: "var(--font-ui)",
                    fontSize: "12px",
                    resize: "none",
                  }}
                />
              </div>
              <div style={{ display: "flex", gap: "6px" }}>
                <button type="submit" className="btn btn-confirm btn-sm" style={{ flex: 1 }}>
                  Save Agent
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={() => {
                    setAgentBuilderOpen(false);
                    setAgentFormStatus("");
                  }}
                >
                  Cancel
                </button>
              </div>
              {agentFormStatus && (
                <div style={{ fontSize: "11px", color: agentFormStatus.startsWith("Save failed") ? "var(--accent-danger)" : "var(--accent-primary)" }}>
                  {agentFormStatus}
                </div>
              )}
            </form>
          )}
        </div>

        <div className="section">
          <div className="section-title">WhatsApp</div>
          {!whatsappOpen ? (
            <button
              className="btn btn-ghost"
              style={{ width: "100%", marginBottom: "8px" }}
              onClick={() => {
                setWhatsappOpen(true);
                refreshWhatsAppStatus();
                if (whatsappStatus.state === "qr_ready" || whatsappStatus.state === "connecting") {
                  fetchWhatsAppQr();
                }
              }}
            >
              + Connect WhatsApp
            </button>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
              <div style={{
                padding: "8px",
                background: "var(--surface-2)",
                borderRadius: "var(--radius-sm)",
                border: "1px solid var(--border-default)",
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "4px" }}>
                  <span className={`status-dot ${whatsappStatus.state === "connected" ? "connected" : "disconnected"}`} />
                  <span style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
                {whatsappStatus.state === "connected" ? `Connected: ${whatsappStatus.phone || "Unknown"}` :
                     whatsappStatus.state === "connecting" ? "Connecting..." :
                     whatsappStatus.state === "qr_ready" ? "Scan QR to connect" :
                     whatsappStatus.state === "sidecar_offline" ? "Sidecar offline" :
                     "Disconnected"}
                  </span>
                </div>
                {whatsappStatus.error && (
                  <div className="field-hint error">{whatsappStatus.error}</div>
                )}
                {whatsappStatus.state === "qr_ready" && whatsappQr && (
                  <div style={{ marginTop: "8px", textAlign: "center" }}>
                    <img
                      src={whatsappQr}
                      alt="WhatsApp QR Code"
                      style={{ width: "150px", height: "150px", borderRadius: "8px" }}
                    />
                    <p style={{ fontSize: "10px", color: "var(--text-secondary)", marginTop: "4px" }}>
                      Scan with WhatsApp app
                    </p>
                  </div>
                )}
              </div>
              <div style={{ display: "flex", gap: "6px" }}>
                {whatsappStatus.state !== "connected" && (
                  <button
                    className="btn btn-confirm btn-sm"
                    style={{ flex: 1 }}
                    onClick={connectWhatsApp}
                  >
                    {whatsappStatus.state === "connecting" ? "Connecting..." : "Connect"}
                  </button>
                )}
                {whatsappStatus.state === "connected" && (
                  <button
                    className="btn btn-danger btn-sm"
                    style={{ flex: 1 }}
                    onClick={disconnectWhatsApp}
                  >
                    Disconnect
                  </button>
                )}
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={resetWhatsApp}
                >
                  Reset
                </button>
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={() => {
                    setWhatsappOpen(false);
                    setWhatsappQr(null);
                  }}
                >
                  Close
                </button>
              </div>
              {whatsappStatus.state === "sidecar_offline" && (
                <div style={{ fontSize: "10px", color: "var(--accent-warning)", marginTop: "4px" }}>
                  Sidecar offline. Restart Agent1 so startup can install dependencies and launch it automatically.
                </div>
              )}
            </div>
          )}
        </div>
      </aside>

      {/* ===== CENTER CANVAS ===== */}
      <main className="center-canvas">
        <CanvasGraph
          agents={supportAgents}
          mcpServers={trace.mcp_servers}
          runningAgentIds={runningAgentIds}
          selectedAgentId={selectedAgentId}
          onSelectAgent={(id) => {
            setSelectedAgentId(id);
          }}
          agent1Agent={agent1Agent}
          onOpenAgent1Config={() => setAgent1ConfigOpen(true)}
          localAgentCount={localAgentCount}
          externalServerCount={externalServerCount}
          connectedExternalCount={connectedExternalCount}
          activeSessionCount={activeSessions.length}
          wsLabel={wsLabel}
          wsState={wsState}
        />

        {agent1ConfigOpen && (
          <div className="agent1-config-popover">
            <form onSubmit={saveAgent1Config}>
              <div className="popover-head">
                <div>
                  <div className="eyebrow">Agent1 Configuration</div>
                  <strong>Primary Orchestrator</strong>
                </div>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={() => {
                    setAgent1ConfigOpen(false);
                    setAgent1FormStatus("");
                  }}
                >
                  Close
                </button>
              </div>
              <div className="agent1-config-grid">
                <div className="form-group">
                  <label>Provider</label>
                  <select
                    value={agent1Form.provider}
                    onChange={(e) => {
                      const provider = e.target.value;
                      const nextProvider = providerOptions.find((item) => item.provider === provider);
                      setAgent1Form({
                        provider,
                        model: nextProvider?.models?.[0]?.name || "",
                      });
                    }}
                  >
                    {providerOptions.map((provider) => (
                      <option key={provider.provider} value={provider.provider}>
                        {provider.label}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="form-group">
                  <label>Model</label>
                  <select
                    value={agent1Form.model}
                    onChange={(e) => setAgent1Form((form) => ({ ...form, model: e.target.value }))}
                    disabled={!agent1ModelOptions.length}
                  >
                    {!agent1ModelOptions.length && <option value="">No models loaded</option>}
                    {agent1ModelOptions.map((model) => (
                      <option key={`${selectedAgent1Provider?.provider}-${model.name}`} value={model.name}>
                        {model.name}
                      </option>
                    ))}
                  </select>
                  {agent1ModelProviderError && (
                    <div className="field-hint error">{agent1ModelProviderError}</div>
                  )}
                </div>
              </div>
              <div className="agent1-capability-strip">
                <span>Local files</span>
                <span>Web/MCP</span>
                <span>Tool creation</span>
                <span>Worker agents</span>
              </div>
              <div className="popover-actions">
                <span className={agent1FormStatus.startsWith("Save failed") ? "field-hint error" : "field-hint"}>
                  {agent1FormStatus || (agent1Agent ? `Using ${agent1Agent.id}` : "Not saved yet")}
                </span>
                <button type="submit" className="btn btn-confirm btn-sm">
                  Save Agent1
                </button>
              </div>
            </form>
          </div>
        )}

        {/* Task Input Bar */}
        <div className="task-input-bar">
          <form ref={runFormRef} onSubmit={runAgent}>
            <div className="composer-header">
              <div>
                <div className="eyebrow">Command Composer</div>
                <strong>Talk to Agent1</strong>
              </div>
              <span className="char-count">{runForm.input.length} chars</span>
            </div>
            <div className="composer-fields">
              <input
                type="text"
                placeholder="Workspace path (e.g., . or C:\projects)"
                value={runForm.workspace}
                onChange={(e) => setRunForm((f) => ({ ...f, workspace: e.target.value }))}
              />
            </div>
            <div className={`composer-thread ${conversation.length === 0 ? "empty" : ""}`}>
              {conversation.length === 0 ? (
                <div className="composer-empty">
                  <span className="empty-icon">A1</span>
                  <span>Commands go directly to Agent1.</span>
                </div>
              ) : (
                conversation.slice(-6).map((message) => (
                  <div key={message.id} className={`chat-line ${message.role.replace(/\s+/g, "-")}`}>
                    <span>{message.role === "user" ? "You" : "Agent1"}</span>
                    <p>{message.content}</p>
                  </div>
                ))
              )}
            </div>
            <textarea
              rows={3}
              placeholder="Message Agent1. It will respond directly or coordinate local workers and external agents."
              value={runForm.input}
              onChange={(e) => setRunForm((f) => ({ ...f, input: e.target.value }))}
            />
            <div className="task-input-actions">
              <span className="composer-hint">Agent1 is the only user-facing agent.</span>
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
                  disabled={isRunning || !runForm.input.trim()}
                >
                  {isRunning ? "Thinking..." : "Send"}
                </button>
              </div>
            </div>
          </form>
        </div>
      </main>

      {/* ===== RIGHT PANEL ===== */}
      <aside className="panel right-panel">
        <div className="activity-section">
          <div className="activity-title">
            External Agents
            <span className="activity-count">{connectedExternalCount}/{externalServerCount}</span>
          </div>
          <button
            className="btn btn-primary"
            style={{ width: "100%", marginBottom: "10px" }}
            onClick={() => setExternalBuilderOpen(true)}
          >
            + Invite External Agent
          </button>

          {externalBuilderOpen && (
            <form className="external-agent-form" onSubmit={createExternalAgent}>
              <div className="form-group">
                <label>Name</label>
                <input
                  type="text"
                  placeholder="filesystem-mcp"
                  value={externalForm.name}
                  onChange={(e) => setExternalForm((form) => ({ ...form, name: e.target.value }))}
                />
              </div>
              <div className="form-group">
                <label>Command</label>
                <input
                  type="text"
                  placeholder="npx"
                  value={externalForm.command}
                  onChange={(e) => setExternalForm((form) => ({ ...form, command: e.target.value }))}
                />
              </div>
              <div className="form-group">
                <label>Arguments</label>
                <input
                  type="text"
                  placeholder="-y @modelcontextprotocol/server-filesystem ."
                  value={externalForm.args}
                  onChange={(e) => setExternalForm((form) => ({ ...form, args: e.target.value }))}
                />
              </div>
              <div className="form-row">
                <label>
                  <input
                    type="checkbox"
                    checked={externalForm.enabled}
                    onChange={(e) => setExternalForm((form) => ({ ...form, enabled: e.target.checked }))}
                  />
                  Start enabled
                </label>
              </div>
              <div className="external-actions">
                <button type="submit" className="btn btn-confirm btn-sm">Invite</button>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={() => {
                    setExternalBuilderOpen(false);
                    setExternalFormStatus("");
                  }}
                >
                  Cancel
                </button>
              </div>
              {externalFormStatus && (
                <div className={externalFormStatus.startsWith("Invite failed") ? "field-hint error" : "field-hint"}>
                  {externalFormStatus}
                </div>
              )}
            </form>
          )}

          {(trace.mcp_servers || []).length === 0 ? (
            <div className="empty-card">
              <span className="empty-icon">EXT</span>
              <strong>No external agents connected.</strong>
              <span>Invite an MCP server or external worker for Agent1 to coordinate.</span>
            </div>
          ) : (
            <div className="external-agent-list">
              {(trace.mcp_servers || []).map((server) => (
                <div key={server.id || server.name} className={`external-agent-card ${server.enabled ? "enabled" : "paused"}`}>
                  <div
                    className="external-avatar"
                    style={{ color: getServerColor(server.name), borderColor: `${getServerColor(server.name)}55` }}
                  >
                    {getAgentInitials(server.name)}
                  </div>
                  <div className="external-info">
                    <strong>{server.name}</strong>
                    <span>{server.enabled ? "connected" : "paused"} / {server.tools?.length || 0} tools</span>
                  </div>
                  <div className="external-card-actions">
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => updateExternalAgent(server, !server.enabled)}
                    >
                      {server.enabled ? "Pause" : "Resume"}
                    </button>
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => removeExternalAgent(server)}
                    >
                      Remove
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="activity-section">
          <div className="activity-title">Connection State</div>
          <div className="external-permission-grid">
            <div>
              <span>Permissions</span>
              <strong>Managed by Agent1</strong>
            </div>
            <div>
              <span>Status</span>
              <strong>{connectedExternalCount > 0 ? "Available" : "Standby"}</strong>
            </div>
          </div>
        </div>

        <div className="activity-section">
          <div className="activity-title">
            Pending Approvals
            <span className="activity-count">{pendingApprovals.length}</span>
          </div>
          {pendingApprovals.length === 0 ? (
            <div className="empty-card">
              <span className="empty-icon">OK</span>
              <strong>No pending approvals.</strong>
              <span>External tool requests that need a decision will appear here.</span>
            </div>
          ) : (
            <div>
              {pendingApprovals.map((approval) => (
                <div key={approval.id} className="approval-item">
                  <div className="approval-header">{approval.id}</div>
                  <div className="approval-request">
                    {approval.request?.tool_name || approval.request?.tool || "tool"} -{" "}
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
      <div className="status-bar">
        {status || "System ready"} / {trace.session?.status || "idle"} / WS {wsState}
      </div>
    </div>
  );
}
