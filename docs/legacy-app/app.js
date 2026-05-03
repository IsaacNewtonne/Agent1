const demoTrace = {
  session: {
    id: "sess_demo",
    title: "Build a local-first autonomous agent app",
    root_agent_id: "assistant",
    status: "completed",
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  },
  messages: [
    {
      role: "user",
      from_agent_id: null,
      content: "Build the most powerful local agent app.",
      created_at: new Date().toISOString(),
    },
    {
      role: "assistant",
      from_agent_id: "assistant",
      content: "{\"tool_call\":{\"name\":\"workspace_search\",\"input\":{\"pattern\":\"MVP Requirements\"}}}",
      created_at: new Date().toISOString(),
    },
    {
      role: "tool",
      from_agent_id: null,
      content: "Tool result: docs and runtime inspected.",
      created_at: new Date().toISOString(),
    },
    {
      role: "assistant",
      from_agent_id: "assistant",
      content: "Runtime expanded with trace export, team orchestration, safer tools, and mission control UI.",
      created_at: new Date().toISOString(),
    },
  ],
  tool_calls: [
    {
      id: "tool_1",
      agent_id: "assistant",
      tool_name: "workspace_search",
      status: "completed",
      input: { pattern: "MVP Requirements" },
      output: { content: "Requirements located.", metadata: { matches_returned: 1 } },
      started_at: new Date().toISOString(),
      finished_at: new Date().toISOString(),
    },
  ],
  events: [
    { event_type: "session_started", agent_id: "assistant", payload: {}, created_at: new Date().toISOString() },
    { event_type: "memory_read", agent_id: "assistant", payload: { count: 2 }, created_at: new Date().toISOString() },
    { event_type: "agent_handoff_completed", agent_id: "assistant", payload: { child_session_id: "sess_worker" }, created_at: new Date().toISOString() },
    {
      event_type: "tool_approval_requested",
      agent_id: "assistant",
      payload: {
        approval_id: "approval_demo",
        tool_name: "workspace_search",
        input: { pattern: "MVP Requirements", path: "Agent1_Project_Docs" },
      },
      created_at: new Date().toISOString(),
    },
    { event_type: "tool_call_completed", agent_id: "assistant", payload: { tool: "workspace_search" }, created_at: new Date().toISOString() },
    { event_type: "final_answer", agent_id: "assistant", payload: { bytes: 112 }, created_at: new Date().toISOString() },
  ],
  agents: [
    { id: "assistant", name: "Assistant", description: "General local assistant" },
    { id: "worker", name: "Worker", description: "Implementation worker" },
  ],
  mcp_servers: [{ name: "filesystem", enabled: true, transport: "stdio" }],
  approvals: [],
  model_providers: [],
};

const SETTINGS_KEY = "agent1_ui_settings_v1";
const defaultSettings = {
  apiBase: "http://127.0.0.1:17371",
  refreshMs: 2000,
  autoRefresh: true,
};

const state = {
  trace: demoTrace,
  settings: loadSettings(),
  refreshTimer: null,
  selectedApproval: null,
  ws: null,
};
state.apiBase = state.settings.apiBase;

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => Array.from(document.querySelectorAll(selector));

$("#demoButton").addEventListener("click", () => {
  state.trace = demoTrace;
  render();
});

$("#apiButton").addEventListener("click", refreshApi);

$("#traceFile").addEventListener("change", async (event) => {
  const [file] = event.target.files;
  if (!file) return;
  const text = await file.text();
  state.trace = JSON.parse(text);
  render();
});

$("#settingsForm").addEventListener("submit", async (event) => {
  event.preventDefault();
  const apiBase = $("#apiBaseInput").value.trim() || defaultSettings.apiBase;
  const refreshMs = Math.max(500, Number($("#refreshMsInput").value || defaultSettings.refreshMs));
  const autoRefresh = $("#autoRefreshInput").checked;
  state.settings = { apiBase, refreshMs, autoRefresh };
  state.apiBase = apiBase;
  saveSettings(state.settings);
  connectWs();
  startAutoRefresh();
  await refreshApi();
});

$("#agentBuilderForm").addEventListener("submit", async (event) => {
  event.preventDefault();
  const payload = {
    id: $("#agentIdInput").value.trim(),
    name: $("#agentNameInput").value.trim(),
    system_prompt: $("#agentPromptInput").value.trim(),
    tools: ["file_read", "file_list", "workspace_search"],
    model: {
      provider: $("#agentProviderInput").value.trim(),
      model: $("#agentModelInput").value.trim(),
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
  const statusEl = $("#agentBuilderStatus");
  try {
    await fetchJson("/api/agents", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    statusEl.textContent = `Saved agent ${payload.id}`;
    await refreshApi();
  } catch (error) {
    statusEl.textContent = `Save failed: ${error.message}`;
  }
});

$$(".tab").forEach((button) => {
  button.addEventListener("click", () => {
    $$(".tab").forEach((tab) => tab.classList.toggle("is-active", tab === button));
    const view = button.dataset.view;
    $$(".trace-view").forEach((panel) => panel.classList.add("is-hidden"));
    $(`#${view}View`).classList.remove("is-hidden");
  });
});

document.addEventListener("click", async (event) => {
  const openButton = event.target.closest("[data-approval-open]");
  if (openButton) {
    const id = openButton.dataset.approvalOpen;
    const approval = (state.trace.approvals || []).find((item) => item.id === id);
    if (approval) openApprovalModal(approval);
    return;
  }
  const quickDecision = event.target.closest("[data-approval-decision]");
  if (!quickDecision) return;
  const id = quickDecision.dataset.approvalId;
  const decision = quickDecision.dataset.approvalDecision;
  await submitApprovalDecision(id, decision);
});

$("#approvalModalClose").addEventListener("click", closeApprovalModal);
$("#approvalApprove").addEventListener("click", async () => {
  if (!state.selectedApproval) return;
  await submitApprovalDecision(state.selectedApproval.id, "approved");
  closeApprovalModal();
});
$("#approvalDeny").addEventListener("click", async () => {
  if (!state.selectedApproval) return;
  await submitApprovalDecision(state.selectedApproval.id, "denied");
  closeApprovalModal();
});

function openApprovalModal(approval) {
  state.selectedApproval = approval;
  const request = approval.request || {};
  const toolName = request.tool_name || request.tool || "tool";
  $("#approvalSummary").textContent = `Agent ${approval.agent_id} requests ${toolName}`;
  $("#approvalPayload").textContent = JSON.stringify(request, null, 2);
  $("#approvalModal").showModal();
}

function closeApprovalModal() {
  state.selectedApproval = null;
  if ($("#approvalModal").open) $("#approvalModal").close();
}

async function submitApprovalDecision(id, decision) {
  if (state.ws && state.ws.readyState === WebSocket.OPEN) {
    state.ws.send(
      JSON.stringify({
        type: "approval_decision",
        approval_id: id,
        decision,
      }),
    );
    setTimeout(() => {
      refreshApi().catch(() => {});
    }, 150);
    return;
  }
  await fetchJson(`/api/tool-approvals/${encodeURIComponent(id)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ decision }),
  });
  await refreshApi();
}

async function refreshApi() {
  const [agents, sessions, events, mcpServers, approvals, models] = await Promise.all([
    fetchJson("/api/agents"),
    fetchJson("/api/sessions"),
    fetchJson("/api/events"),
    fetchJson("/api/mcp/servers"),
    fetchJson("/api/approvals"),
    fetchJson("/api/models"),
  ]);
  const latestSession = (sessions.sessions || [])[0] || {};
  const trace = latestSession.id
    ? await fetchJson(`/api/sessions/${encodeURIComponent(latestSession.id)}/trace`)
    : { session: latestSession, messages: [], tool_calls: [], events: [], approvals: [] };
  state.trace = {
    ...trace,
    events: trace.events && trace.events.length ? trace.events : events.events || [],
    agents: agents.agents || [],
    mcp_servers: mcpServers.servers || [],
    approvals: trace.approvals || approvals.approvals || [],
    model_providers: models.providers || [],
  };
  render();
}

async function fetchJson(path, options) {
  const response = await fetch(`${state.apiBase}${path}`, options);
  if (!response.ok) throw new Error(`${path} returned ${response.status}`);
  return response.json();
}

function render() {
  const trace = normalizeTrace(state.trace);
  const agents = collectAgents(trace);
  const approvalRecords = trace.approvals || [];
  const pendingApprovals = approvalRecords.filter((item) => !item.decision);
  const memoryEvents = trace.events.filter((event) => event.event_type && event.event_type.startsWith("memory_"));
  const handoffEvents = trace.events.filter((event) => event.event_type && event.event_type.startsWith("agent_handoff"));
  const lastEvent = trace.events.at(-1);

  applySettingsToInputs();
  $("#sessionStatus").textContent = trace.session.status || "unknown";
  $("#sessionId").textContent = trace.session.id || "No trace loaded";
  $("#sessionTitle").textContent = trace.session.title || "Untitled session";
  $("#rootAgent").textContent = trace.session.root_agent_id || "Unknown agent";
  $("#messageCount").textContent = trace.messages.length;
  $("#toolCount").textContent = trace.tool_calls.length;
  $("#eventCount").textContent = trace.events.length;
  $("#lastEvent").textContent = lastEvent ? lastEvent.event_type : "silent";
  $("#riskSummary").textContent = approvalRecords.length
    ? `${approvalRecords.length} approval request${approvalRecords.length === 1 ? "" : "s"} recorded`
    : "No approvals requested";

  $("#agentGraph").innerHTML = agents.map((agent) => agentNode(agent, trace.session.root_agent_id)).join("");
  $("#approvalList").innerHTML = pendingApprovals.length
    ? pendingApprovals.map((approval) => `<span class="approval-chip">${escapeHtml(approval.request?.tool_name || "tool")} pending</span>`).join("")
    : `<span class="approval-chip">locked down</span>`;
  $("#messagesView").innerHTML = trace.messages.length ? trace.messages.map(messageCard).join("") : empty("No messages in this trace.");
  $("#toolsView").innerHTML = trace.tool_calls.length ? trace.tool_calls.map(toolCard).join("") : empty("No tool calls in this trace.");
  $("#memoryView").innerHTML = memoryEvents.length ? memoryEvents.map(eventCard).join("") : empty("No memory reads or writes in this trace.");
  $("#agentsView").innerHTML = [
    trace.agents.length ? trace.agents.map(agentCard).join("") : empty("No saved agents loaded."),
    trace.model_providers.length ? `<div class="section-label">Models</div>${trace.model_providers.map(modelProviderCard).join("")}` : "",
    trace.mcp_servers.length ? `<div class="section-label">MCP Servers</div>${trace.mcp_servers.map(mcpCard).join("")}` : empty("No MCP servers loaded."),
    trace.approvals.length ? `<div class="section-label">Approvals</div>${trace.approvals.map(approvalCard).join("")}` : "",
    handoffEvents.length ? `<div class="section-label">Handoffs</div>${handoffEvents.map(eventCard).join("")}` : "",
  ].join("");
  $("#eventsView").innerHTML = trace.events.length ? trace.events.map(eventCard).join("") : empty("No events in this trace.");
  $("#eventFeed").innerHTML = trace.events.slice(-20).reverse().map(eventCard).join("") || empty("No event feed yet.");
}

function startAutoRefresh() {
  if (state.refreshTimer) clearInterval(state.refreshTimer);
  if (!state.settings.autoRefresh) return;
  state.refreshTimer = setInterval(() => {
    refreshApi().catch(() => {});
  }, state.settings.refreshMs);
}

function connectWs() {
  if (state.ws) {
    try {
      state.ws.close();
    } catch {}
  }
  const wsBase = state.settings.apiBase
    .replace(/^http:\/\//i, "ws://")
    .replace(/^https:\/\//i, "wss://");
  const ws = new WebSocket(`${wsBase}/ws/events`);
  state.ws = ws;

  ws.addEventListener("message", (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch {
      return;
    }
    if (message.type === "event" && message.event) {
      const eventId = message.event.id;
      const events = state.trace.events || [];
      if (!events.some((item) => item.id === eventId)) {
        state.trace.events = [...events, message.event];
        render();
      }
      return;
    }
    if (message.type === "approval_decision_ack") {
      refreshApi().catch(() => {});
    }
  });

  ws.addEventListener("close", () => {
    if (state.ws === ws) {
      state.ws = null;
      setTimeout(() => {
        if (!state.ws) connectWs();
      }, 1200);
    }
  });
}

function applySettingsToInputs() {
  $("#apiBaseInput").value = state.settings.apiBase;
  $("#refreshMsInput").value = String(state.settings.refreshMs);
  $("#autoRefreshInput").checked = Boolean(state.settings.autoRefresh);
}

function normalizeTrace(trace) {
  return {
    session: trace.session || {},
    messages: trace.messages || [],
    tool_calls: trace.tool_calls || trace.toolCalls || [],
    events: trace.events || [],
    agents: trace.agents || [],
    mcp_servers: trace.mcp_servers || trace.mcpServers || [],
    approvals: trace.approvals || [],
    model_providers: trace.model_providers || trace.modelProviders || [],
  };
}

function collectAgents(trace) {
  const ids = new Set([trace.session.root_agent_id].filter(Boolean));
  trace.messages.forEach((message) => {
    if (message.from_agent_id) ids.add(message.from_agent_id);
    if (message.to_agent_id) ids.add(message.to_agent_id);
  });
  trace.events.forEach((event) => event.agent_id && ids.add(event.agent_id));
  trace.tool_calls.forEach((call) => call.agent_id && ids.add(call.agent_id));
  return Array.from(ids).map((id) => ({
    id,
    role: id === trace.session.root_agent_id ? "root agent" : "participant",
  }));
}

function agentCard(agent) {
  return `
    <article class="tool-card">
      <header>
        <span>${escapeHtml(agent.id || agent.name || "agent")}</span>
        <span>${escapeHtml(agent.name || "")}</span>
      </header>
      <pre>${escapeHtml(agent.description || agent.role || "No description")}</pre>
    </article>
  `;
}

function mcpCard(server) {
  return `
    <article class="tool-card">
      <header>
        <span>${escapeHtml(server.name || server.id || "mcp")}</span>
        <span>${server.enabled ? "enabled" : "disabled"}</span>
      </header>
      <pre>${escapeHtml(JSON.stringify({ transport: server.transport, command: server.command, args: server.args }, null, 2))}</pre>
    </article>
  `;
}

function modelProviderCard(provider) {
  return `
    <article class="tool-card">
      <header>
        <span>${escapeHtml(provider.provider || "provider")}</span>
        <span>${provider.error ? "error" : "available"}</span>
      </header>
      <pre>${escapeHtml(provider.error || JSON.stringify(provider.models || [], null, 2))}</pre>
    </article>
  `;
}

function approvalCard(approval) {
  const request = approval.request || {};
  const toolName = request.tool_name || request.tool || "tool";
  const pendingControls = approval.decision
    ? ""
    : `<div class="approval-actions">
        <button type="button" data-approval-open="${escapeHtml(approval.id)}">Review exact action</button>
        <button type="button" data-approval-id="${escapeHtml(approval.id)}" data-approval-decision="approved">Approve</button>
        <button type="button" data-approval-id="${escapeHtml(approval.id)}" data-approval-decision="denied">Deny</button>
      </div>`;
  return `
    <article class="tool-card approval-request">
      <header>
        <span>${escapeHtml(approval.id || "approval")}</span>
        <span>${escapeHtml(approval.decision || "pending")}</span>
      </header>
      <pre>${escapeHtml(JSON.stringify({ tool_name: toolName, request }, null, 2))}</pre>
      ${pendingControls}
    </article>
  `;
}

function agentNode(agent, rootId) {
  return `
    <article class="agent-node ${agent.id === rootId ? "is-active" : ""}">
      <strong>${escapeHtml(agent.id)}</strong>
      <span>${escapeHtml(agent.role)}</span>
    </article>
  `;
}

function messageCard(message) {
  return `
    <article class="message">
      <header>
        <span>${escapeHtml(message.role || "message")}</span>
        <span>${escapeHtml(message.from_agent_id || "user")}</span>
      </header>
      <pre>${escapeHtml(message.content || "")}</pre>
    </article>
  `;
}

function toolCard(call) {
  return `
    <article class="tool-card">
      <header>
        <span>${escapeHtml(call.tool_name || "tool")}</span>
        <span>${escapeHtml(call.status || "unknown")}</span>
      </header>
      <pre>${escapeHtml(JSON.stringify({ input: call.input, output: call.output, error: call.error }, null, 2))}</pre>
    </article>
  `;
}

function eventCard(event) {
  return `
    <article class="event-item" data-kind="${escapeHtml(event.event_type || "")}">
      <header>
        <span>${escapeHtml(event.event_type || "event")}</span>
        <span>${formatTime(event.created_at)}</span>
      </header>
      <pre>${escapeHtml(JSON.stringify(event.payload || {}, null, 2))}</pre>
    </article>
  `;
}

function empty(text) {
  return `<div class="empty-state">${escapeHtml(text)}</div>`;
}

function formatTime(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return escapeHtml(value);
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function loadSettings() {
  try {
    const parsed = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}");
    return {
      apiBase: parsed.apiBase || defaultSettings.apiBase,
      refreshMs: Number(parsed.refreshMs) >= 500 ? Number(parsed.refreshMs) : defaultSettings.refreshMs,
      autoRefresh: parsed.autoRefresh !== false,
    };
  } catch {
    return { ...defaultSettings };
  }
}

function saveSettings(settings) {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
}

render();
startAutoRefresh();
connectWs();
