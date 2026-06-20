import { useState, useMemo, useCallback, useEffect } from "react";
import useCollaboration from "../hooks/useCollaboration";
import {
  ProjectHeader,
  AgentLane,
  ActivityFeed,
  StreamOutput,
  TaskComposer,
} from "./CollabComponents";
import SharedWorkspaceVisual from "./SharedWorkspaceVisual";
import ProjectSphere from "./ProjectSphere";

const SETTINGS_KEY = "agent1_desktop_settings_v1";
const AGENT1_ID = "agent1";
const AGENT1_SYSTEM_PROMPT =
  "You are Agent1, the central orchestrator for this desktop app. Talk directly with the user, coordinate local worker agents, use local files and tools when approved, browse through connected tools, create tools when needed, and manage supporting agents. Keep responses practical, clear, and action-oriented.";
const AGENT1_TOOLS = [
  "file_read", "file_list", "workspace_search", "git_status", "git_diff",
  "task_board", "memory_search", "memory_write", "agent_call", "mcp_call",
];
const AGENT1_PERMISSIONS = {
  file_read: "ask", file_list: "ask", workspace_search: "ask",
  git_status: "ask", git_diff: "ask", task_board: "ask",
  memory_search: "ask", memory_write: "ask", agent_call: "ask", mcp_call: "ask",
  file_write: "deny", shell: "deny",
};
const MODEL_PROVIDER_LABELS = {
  ollama: "Ollama",
  opencode: "OpenCode",
  openai_compatible: "OpenAI-compatible",
  nvidia: "NVIDIA NIM",
};

function loadSettings() {
  try {
    const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}");
    return {
      apiBase: saved.apiBase || "http://127.0.0.1:17371",
      refreshMs: Number(saved.refreshMs) >= 500 ? Number(saved.refreshMs) : 2000,
      autoRefresh: saved.autoRefresh !== false,
    };
  } catch {
    return { apiBase: "http://127.0.0.1:17371", refreshMs: 2000, autoRefresh: true };
  }
}

function modelName(model) {
  return typeof model === "string" ? model : model?.name || "";
}

function formatEventName(type = "") {
  return String(type).replace(/([A-Z])/g, " $1").trim() || "Event";
}

function displayModelContent(content) {
  if (typeof content !== "string") return String(content ?? "");
  const trimmed = content.trim();
  if (!trimmed.startsWith("{")) return content;
  try {
    const parsed = JSON.parse(trimmed);
    if (typeof parsed.final === "string") return parsed.final;
  } catch {
    // Not model-action JSON; show the original content.
  }
  return content;
}

function gatewayUrl(apiBase, token, agentName = "hermes-agent") {
  if (!token) return "";
  const base = String(apiBase || "http://127.0.0.1:17371").replace(/^http/i, "ws").replace(/\/$/, "");
  return `${base}/gateway/connect?token=${encodeURIComponent(token)}&agent_name=${encodeURIComponent(agentName)}`;
}

const POLICY_PROFILES = {
  automatic: {
    label: "Automatic",
    posture: "Balanced",
    rules: ["Context-based routing", "Default tool policy", "Standard audit trail"],
  },
  structured: {
    label: "Structured",
    posture: "Planned",
    rules: ["Plan before execution", "Delegate then review", "Task trace preferred"],
  },
  fast: {
    label: "Fast",
    posture: "High throughput",
    rules: ["Parallel execution", "Minimal interruption", "Review after completion"],
  },
  careful: {
    label: "Careful",
    posture: "Approval-first",
    rules: ["Ask before risky tools", "Human review expected", "Detailed approval trail"],
  },
  enterprise: {
    label: "Enterprise",
    posture: "Audit-heavy",
    rules: ["Approval trail required", "External access reviewed", "Security signals monitored"],
  },
  airgapped: {
    label: "Airgapped",
    posture: "Local-only",
    rules: ["Disable external agents", "Avoid network tools", "Prefer local models and MCP"],
  },
};

function projectPolicy(project) {
  return POLICY_PROFILES[project?.collaboration_mode || "automatic"] || POLICY_PROFILES.automatic;
}

function buildSecuritySignals({ approvals, events, externals, mcpServers }) {
  const pendingApprovals = approvals.filter((approval) => !approval.decision).length;
  const deniedApprovals = approvals.filter((approval) => approval.decision === "denied").length;
  const failedEvents = events.filter((event) => /fail|error|crash/i.test(`${event.event_type} ${JSON.stringify(event.payload || {})}`)).length;
  const disconnectedExternals = externals.filter((agent) => agent.status && agent.status !== "connected").length;
  const disabledMcp = mcpServers.filter((server) => !server.enabled).length;
  return [
    { label: "Pending approvals", value: pendingApprovals, severity: pendingApprovals ? "warn" : "ok" },
    { label: "Denied actions", value: deniedApprovals, severity: deniedApprovals ? "warn" : "ok" },
    { label: "Failed events", value: failedEvents, severity: failedEvents ? "danger" : "ok" },
    { label: "External drift", value: disconnectedExternals, severity: disconnectedExternals ? "warn" : "ok" },
    { label: "Disabled MCP", value: disabledMcp, severity: disabledMcp ? "warn" : "ok" },
  ];
}

function ProjectInspector({ collab, apiBase, onClose }) {
  const trace = collab.trace || {};
  const session = trace.session || {};
  const messages = trace.messages || [];
  const events = trace.events || [];
  const approvals = trace.approvals || [];
  const tasks = collab.collabTasks || [];
  const blackboard = collab.blackboardEntries || [];
  const policy = projectPolicy(collab.activeProject);
  const signals = buildSecuritySignals({
    approvals,
    events,
    externals: collab.externals || [],
    mcpServers: collab.mcpServers || [],
  });
  const [secureSendState, setSecureSendState] = useState({ status: "idle", result: null, error: "" });
  const [stripeState, setStripeState] = useState({ status: "idle", result: null, error: "" });
  const secureSendReady = Boolean(session.id) && Boolean(collab.crocStatus?.available);
  const stripeMode = collab.stripeStatus?.mode || "dry_run";

  const handleSecureSend = async () => {
    if (!session.id || secureSendState.status === "sending") return;
    setSecureSendState({ status: "sending", result: null, error: "" });
    try {
      const result = await collab.secureSendSession(session.id, { format: "markdown" });
      setSecureSendState({ status: "sent", result, error: "" });
    } catch (error) {
      setSecureSendState({ status: "error", result: null, error: error.message });
    }
  };

  const handleCopyReceive = async () => {
    const command = secureSendState.result?.receiver_command;
    if (!command) return;
    try {
      await navigator.clipboard.writeText(command);
      setSecureSendState((state) => ({ ...state, status: "copied" }));
    } catch {
      setSecureSendState((state) => ({ ...state, status: "sent" }));
    }
  };

  const handleCreateCheckout = async () => {
    if (stripeState.status === "creating") return;
    setStripeState({ status: "creating", result: null, error: "" });
    try {
      const result = await collab.createStripeCheckout({
        name: "Agent1 Revenue Ops Retainer",
        description: "AI operations workspace managed by Agent1 agents.",
        amount_cents: 4900,
        currency: "usd",
      });
      setStripeState({ status: "created", result, error: "" });
    } catch (error) {
      setStripeState({ status: "error", result: null, error: error.message });
    }
  };

  const handleCopyCheckout = async () => {
    const url = stripeState.result?.checkout_session?.url;
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      setStripeState((state) => ({ ...state, status: "copied" }));
    } catch {
      setStripeState((state) => ({ ...state, status: "created" }));
    }
  };

  return (
    <aside className="project-inspector glass-panel" role="dialog" aria-label="Project inspector">
      <div className="inspector-head">
        <div>
          <span className="popover-sub">Project timeline</span>
          <strong>{collab.activeProject?.name || "No project"}</strong>
        </div>
        <button type="button" className="btn-ghost" onClick={onClose}>Close</button>
      </div>

      <div className="inspector-grid">
        <section>
          <div className="collab-section-label">SESSION</div>
          <div className="inspector-kv">
            <span>ID</span><strong>{session.id || "none"}</strong>
            <span>Status</span><strong>{session.status || "idle"}</strong>
            <span>Agent</span><strong>{session.root_agent_id || collab.agent1Agent?.id || "agent1"}</strong>
            <span>Project</span><strong>{session.project_id || collab.activeProject?.id || "unbound"}</strong>
          </div>
          <div className="secure-send-card">
            <div>
              <span className={`secure-send-dot ${collab.crocStatus?.available ? "ready" : "missing"}`} />
              <strong>{collab.crocStatus?.available ? "croc ready" : "croc unavailable"}</strong>
            </div>
            <button
              type="button"
              className="btn-confirm small"
              disabled={!secureSendReady || secureSendState.status === "sending"}
              onClick={handleSecureSend}
            >
              {secureSendState.status === "sending" ? "Starting..." : "Secure Send"}
            </button>
            {secureSendState.result?.receiver_command && (
              <div className="secure-send-command">
                <code>{secureSendState.result.receiver_command}</code>
                <button type="button" className="btn-ghost small" onClick={handleCopyReceive}>
                  {secureSendState.status === "copied" ? "Copied" : "Copy"}
                </button>
              </div>
            )}
            {(secureSendState.error || (!collab.crocStatus?.available && collab.crocStatus?.error)) && (
              <span className="field-hint error">
                {secureSendState.error || collab.crocStatus.error}
              </span>
            )}
          </div>
        </section>

        <section>
          <div className="collab-section-label">PROJECT STATE</div>
          <div className="inspector-kv">
            <span>Tasks</span><strong>{tasks.length}</strong>
            <span>Blackboard</span><strong>{blackboard.length}</strong>
            <span>Externals</span><strong>{(collab.externals || []).length}</strong>
            <span>MCP</span><strong>{(collab.mcpServers || []).length}</strong>
          </div>
        </section>
      </div>

      <div className="inspector-columns">
        <section>
          <div className="collab-section-label">POLICY MODE</div>
          <div className="policy-card">
            <strong>{policy.label}</strong>
            <span>{policy.posture}</span>
            <div className="policy-rule-list">
              {policy.rules.map((rule) => <em key={rule}>{rule}</em>)}
            </div>
          </div>
        </section>

        <section>
          <div className="collab-section-label">SECURITY SIGNALS</div>
          <div className="security-signal-grid">
            {signals.map((signal) => (
              <div key={signal.label} className={`security-signal ${signal.severity}`}>
                <strong>{signal.value}</strong>
                <span>{signal.label}</span>
              </div>
            ))}
          </div>
        </section>
      </div>

      <div className="inspector-columns">
        <section>
          <div className="collab-section-label">REVENUE OPS</div>
          <div className="ops-card stripe">
            <div className="ops-card-head">
              <div>
                <strong>Stripe Checkout</strong>
                <span>{collab.stripeStatus?.configured ? "Live API ready" : "Dry-run mode"}</span>
              </div>
              <button
                type="button"
                className="btn-confirm small"
                onClick={handleCreateCheckout}
                disabled={!collab.activeProject || stripeState.status === "creating"}
              >
                {stripeState.status === "creating" ? "Creating..." : "Create Checkout"}
              </button>
            </div>
            <div className="ops-kv">
              <span>Mode</span><strong>{stripeMode}</strong>
              <span>Amount</span><strong>$49.00 USD</strong>
              <span>Project</span><strong>{collab.activeProject?.name || "none"}</strong>
            </div>
            {stripeState.result?.checkout_session?.url && (
              <div className="ops-command">
                <code>{stripeState.result.checkout_session.url}</code>
                <button type="button" className="btn-ghost small" onClick={handleCopyCheckout}>
                  {stripeState.status === "copied" ? "Copied" : "Copy"}
                </button>
              </div>
            )}
            {stripeState.error && <span className="field-hint error">{stripeState.error}</span>}
          </div>
        </section>

        <section>
          <div className="collab-section-label">HERMES GATEWAY</div>
          <div className="ops-card hermes">
            <div className="ops-card-head">
              <div>
                <strong>Local or External Hermes</strong>
                <span>Project-scoped WebSocket agent access</span>
              </div>
            </div>
            <div className="ops-kv">
              <span>Gateway</span><strong>{apiBase.replace(/^http/i, "ws")}/gateway/connect</strong>
              <span>Open invites</span><strong>{(collab.inviteTokens || []).filter((invite) => !invite.used_by).length}</strong>
              <span>Permissions</span><strong>blackboard, tasks, artifacts</strong>
            </div>
          </div>
        </section>
      </div>

      <div className="inspector-columns">
        <section>
          <div className="collab-section-label">MESSAGES</div>
          <div className="inspector-list">
            {messages.slice(-8).map((message) => (
              <article key={message.id} className="inspector-row">
                <strong>{message.role}</strong>
                <p>{displayModelContent(message.content).slice(0, 360)}</p>
              </article>
            ))}
            {!messages.length && <div className="inspector-empty">No messages yet.</div>}
          </div>
        </section>

        <section>
          <div className="collab-section-label">EVENTS</div>
          <div className="inspector-list">
            {events.slice(-10).reverse().map((event) => (
              <article key={event.id} className="inspector-row">
                <strong>{formatEventName(event.event_type)}</strong>
                <p>{typeof event.payload === "object" ? JSON.stringify(event.payload) : String(event.payload || "")}</p>
              </article>
            ))}
            {!events.length && <div className="inspector-empty">No runtime events yet.</div>}
          </div>
        </section>
      </div>

      <div className="inspector-columns">
        <section>
          <div className="collab-section-label">TASKS</div>
          <div className="inspector-list compact">
            {tasks.slice(0, 8).map((task) => (
              <article key={task.id} className="inspector-row">
                <strong>{task.status || "queued"}</strong>
                <p>{task.description}</p>
              </article>
            ))}
            {!tasks.length && <div className="inspector-empty">No project tasks.</div>}
          </div>
        </section>

        <section>
          <div className="collab-section-label">BLACKBOARD</div>
          <div className="inspector-list compact">
            {blackboard.slice(0, 8).map((entry) => (
              <article key={entry.id || entry.key} className="inspector-row">
                <strong>{entry.key}</strong>
                <p>{typeof entry.value === "object" ? JSON.stringify(entry.value) : String(entry.value ?? "")}</p>
              </article>
            ))}
            {!blackboard.length && <div className="inspector-empty">No blackboard entries.</div>}
          </div>
        </section>
      </div>

      {approvals.length > 0 && (
        <section>
          <div className="collab-section-label">APPROVAL HISTORY</div>
          <div className="inspector-list compact">
            {approvals.slice(0, 6).map((approval) => (
              <article key={approval.id} className="inspector-row">
                <strong>{approval.decision || "pending"}</strong>
                <p>{approval.request?.tool_name || "approval"} by {approval.agent_id}</p>
              </article>
            ))}
          </div>
        </section>
      )}
    </aside>
  );
}

export default function CollabWorkspace() {
  const settings = loadSettings();
  const collab = useCollaboration(settings.apiBase);

  // Local UI state
  const [selectedAgentId, setSelectedAgentId] = useState(null);
  const [agentBuilderOpen, setAgentBuilderOpen] = useState(false);
  const [externalBuilderOpen, setExternalBuilderOpen] = useState(false);
  const [agent1ConfigOpen, setAgent1ConfigOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [conversation, setConversation] = useState([]);
  const [isRunning, setIsRunning] = useState(false);
  const [deleteCandidate, setDeleteCandidate] = useState(null);
  const [deletingAgentId, setDeletingAgentId] = useState(null);

  // Agent builder state
  const [agentForm, setAgentForm] = useState({
    id: "", name: "", provider: "opencode", model: "",
    systemPrompt: "You are a practical local agent. Be concise and direct.",
    role: "",
  });
  const [agentFormStatus, setAgentFormStatus] = useState("");

  // Agent1 config state
  const [agent1Form, setAgent1Form] = useState({ provider: "opencode", model: "" });
  const [agent1FormStatus, setAgent1FormStatus] = useState("");

  // External server builder state
  const [externalForm, setExternalForm] = useState({ name: "", command: "", args: "", enabled: true });
  const [externalFormStatus, setExternalFormStatus] = useState("");
  const [invitePermissions, setInvitePermissions] = useState({
    can_read_blackboard: true,
    can_write_blackboard: false,
    can_create_artifacts: false,
    can_delegate_tasks: false,
    max_concurrent_tasks: 2,
    allowed_tools: "",
  });
  const [inviteResult, setInviteResult] = useState(null);
  const [mcpActionStatus, setMcpActionStatus] = useState("");
  const [expandedMcpServerId, setExpandedMcpServerId] = useState(null);
  const [mcpToolsByServer, setMcpToolsByServer] = useState({});

  // Derived
  const connectedExternalCount = useMemo(
    () => (
      (collab.externals || []).filter((agent) => agent.status === "connected").length +
      (collab.mcpServers || []).filter((server) => server.enabled).length
    ),
    [collab.externals, collab.mcpServers]
  );
  const providerOptions = useMemo(() => {
    const fallback = ["opencode", "ollama", "openai_compatible", "nvidia"].map((provider) => ({
      provider,
      label: MODEL_PROVIDER_LABELS[provider] || provider,
      models: [],
      error: "",
    }));
    const merged = new Map(fallback.map((item) => [item.provider, item]));
    for (const provider of collab.modelProviders || []) {
      if (!provider?.provider) continue;
      merged.set(provider.provider, {
        ...provider,
        label: MODEL_PROVIDER_LABELS[provider.provider] || provider.provider,
        models: Array.isArray(provider.models) ? provider.models : [],
        error: provider.error || "",
      });
    }
    return [...merged.values()];
  }, [collab.modelProviders]);
  const selectedAgent1Provider =
    providerOptions.find((item) => item.provider === agent1Form.provider) || providerOptions[0];
  const selectedAgentProvider =
    providerOptions.find((item) => item.provider === agentForm.provider) || providerOptions[0];
  const agentModelOptions = useMemo(() => {
    const options = selectedAgentProvider?.models || [];
    if (!agentForm.model) return options;
    if (options.some((model) => modelName(model) === agentForm.model)) return options;
    return [{ name: agentForm.model }, ...options];
  }, [agentForm.model, selectedAgentProvider]);
  const agentModelProviderError = selectedAgentProvider?.error || "";
  const agent1ModelOptions = useMemo(() => {
    const options = selectedAgent1Provider?.models || [];
    if (!agent1Form.model) return options;
    if (options.some((model) => modelName(model) === agent1Form.model)) return options;
    return [{ name: agent1Form.model }, ...options];
  }, [agent1Form.model, selectedAgent1Provider]);
  const agent1ModelProviderError = selectedAgent1Provider?.error || "";
  const activePolicyMode = collab.activeProject?.collaboration_mode || "automatic";
  const isAirgapped = activePolicyMode === "airgapped";
  const hermesInvite = inviteResult || (collab.inviteTokens || []).find((invite) => !invite.used_by) || null;

  // ─── Agent1 config ───

  useEffect(() => {
    if (collab.agent1Agent?.model) {
      setAgent1Form({
        provider: collab.agent1Agent.model.provider || "opencode",
        model: collab.agent1Agent.model.model || "",
      });
    }
  }, [collab.agent1Agent]);

  useEffect(() => {
    if (collab.agent1Agent?.model) return;
    if (!selectedAgent1Provider?.provider) return;
    const firstModel = modelName(selectedAgent1Provider.models?.[0]);
    setAgent1Form((form) => {
      if (form.provider && form.model) return form;
      return {
        provider: form.provider || selectedAgent1Provider.provider,
        model: form.model || firstModel,
      };
    });
  }, [collab.agent1Agent, selectedAgent1Provider]);

  useEffect(() => {
    if (!selectedAgentProvider?.provider) return;
    const firstModel = modelName(selectedAgentProvider.models?.[0]);
    setAgentForm((form) => {
      if (!form.provider) {
        return {
          ...form,
          provider: selectedAgentProvider.provider,
          model: form.model || firstModel,
        };
      }
      if (form.provider !== selectedAgentProvider.provider) {
        return {
          ...form,
          model: firstModel,
        };
      }
      return form;
    });
  }, [selectedAgentProvider]);

  useEffect(() => {
    const onKeyDown = (event) => {
      if (event.key !== "Escape") return;
      setAgentBuilderOpen(false);
      setExternalBuilderOpen(false);
      setAgent1ConfigOpen(false);
      setInspectorOpen(false);
      setDeleteCandidate(null);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const handleSaveAgent1 = async (e) => {
    e.preventDefault();
    if (!agent1Form.provider.trim() || !agent1Form.model.trim()) {
      setAgent1FormStatus("Provider and model are required.");
      return;
    }
    try {
      const payload = {
        ...(collab.agent1Agent || {}),
        id: collab.agent1Agent?.id || AGENT1_ID,
        name: "Agent1",
        description: "Central user-facing orchestrator and controller",
        role: "Orchestrator",
        system_prompt: AGENT1_SYSTEM_PROMPT,
        tools: AGENT1_TOOLS,
        model: {
          provider: agent1Form.provider.trim(),
          model: agent1Form.model.trim(),
          base_url: collab.agent1Agent?.model?.base_url || undefined,
          context_window: collab.agent1Agent?.model?.context_window || 8192,
          temperature: collab.agent1Agent?.model?.temperature ?? 0.2,
        },
        memory: { ...(collab.agent1Agent?.memory || {}), enabled: true },
        permissions: { ...(collab.agent1Agent?.permissions || {}), ...AGENT1_PERMISSIONS },
        max_iterations: collab.agent1Agent?.max_iterations || 16,
      };
      await collab.saveAgent(payload);
      setAgent1FormStatus("Saved!");
      setAgent1ConfigOpen(false);
      setAgent1FormStatus("");
    } catch (err) {
      setAgent1FormStatus(`Error: ${err.message}`);
    }
  };

  // ─── Run task ───

  const handleRunTask = useCallback(async (input, workspace) => {
    if (isRunning) return;
    setIsRunning(true);
    setConversation((prev) => [...prev, { role: "user", content: input }]);
    try {
      const result = await collab.runTask(input, collab.agent1Agent?.id, workspace, collab.activeProject?.id);
      if (result?.final_answer) {
        setConversation((prev) => [...prev, { role: "agent1", content: result.final_answer }]);
      }
    } catch (err) {
      setConversation((prev) => [...prev, { role: "agent1-error", content: `Error: ${err.message}` }]);
    } finally {
      setIsRunning(false);
    }
  }, [isRunning, collab]);

  // ─── Agent builder ───

  const handleSaveAgent = async (e) => {
    e.preventDefault();
    if (!agentForm.name.trim()) { setAgentFormStatus("Name required"); return; }
    if (!agentForm.provider.trim() || !agentForm.model.trim()) { setAgentFormStatus("Model required"); return; }
    try {
      const id = agentForm.id || agentForm.name.toLowerCase().replace(/[^a-z0-9]+/g, "_");
      await collab.saveAgent({
        id,
        name: agentForm.name.trim(),
        description: "",
        role: agentForm.role || null,
        system_prompt: agentForm.systemPrompt,
        model: { provider: agentForm.provider.trim(), model: agentForm.model.trim() },
        tools: ["file_read", "file_list", "workspace_search"],
        memory: { enabled: false },
        permissions: {},
        max_iterations: 10,
      });
      setAgentFormStatus("Created!");
      setTimeout(() => { setAgentFormStatus(""); setAgentBuilderOpen(false); }, 800);
      setAgentForm({ id: "", name: "", provider: "opencode", model: "", systemPrompt: "You are a practical local agent. Be concise and direct.", role: "" });
    } catch (err) {
      setAgentFormStatus(`Error: ${err.message}`);
    }
  };

  const handleDeleteAgent = useCallback(async (agent) => {
    if (!agent?.id || deletingAgentId) return;
    setDeleteCandidate(agent);
  }, [deletingAgentId]);

  const confirmDeleteAgent = useCallback(async () => {
    const agent = deleteCandidate;
    if (!agent?.id || deletingAgentId) return;
    setDeletingAgentId(agent.id);
    try {
      await collab.deleteAgent(agent.id);
      if (selectedAgentId === agent.id) {
        setSelectedAgentId(null);
      }
      setDeleteCandidate(null);
    } catch (err) {
      setAgentFormStatus(`Delete failed: ${err.message}`);
    } finally {
      setDeletingAgentId(null);
    }
  }, [collab, deleteCandidate, deletingAgentId, selectedAgentId]);

  // ─── External server builder ───

  const handleSaveExternal = async (e) => {
    e.preventDefault();
    if (isAirgapped) {
      setExternalFormStatus("Airgapped policy blocks MCP server changes.");
      return;
    }
    if (!externalForm.name.trim() || !externalForm.command.trim()) {
      setExternalFormStatus("Name and command required"); return;
    }
    try {
      const args = externalForm.args ? externalForm.args.split(",").map((s) => s.trim()).filter(Boolean) : [];
      const name = externalForm.name.trim();
      const command = externalForm.command.trim();
      const duplicate = (collab.mcpServers || []).some((server) => (
        server.name?.toLowerCase() === name.toLowerCase() &&
        (server.transport || "stdio") === "stdio" &&
        (server.command || "") === command &&
        JSON.stringify(server.args || []) === JSON.stringify(args)
      ));
      if (duplicate) {
        setExternalFormStatus("Duplicate MCP server already exists");
        return;
      }
      await collab.saveMcpServer({
        name,
        transport: "stdio",
        command,
        args,
        env: {},
        enabled: externalForm.enabled,
        project_id: collab.activeProject?.id,
      });
      setExternalFormStatus("Added!");
      setExternalForm({ name: "", command: "", args: "", enabled: true });
    } catch (err) {
      setExternalFormStatus(`Error: ${err.message}`);
    }
  };

  const handleCreateInvite = async () => {
    setExternalFormStatus("");
    setInviteResult(null);
    if (isAirgapped) {
      setExternalFormStatus("Airgapped policy blocks external invites.");
      return;
    }
    try {
      const permissions = {
        ...invitePermissions,
        allowed_tools: invitePermissions.allowed_tools
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean),
        max_concurrent_tasks: Number(invitePermissions.max_concurrent_tasks) || 1,
      };
      const result = await collab.createInvite(permissions);
      setInviteResult(result.invite || result);
    } catch (err) {
      setExternalFormStatus(`Invite failed: ${err.message}`);
    }
  };

  const handleCopyInvite = async (invite) => {
    const token = invite?.token;
    if (!token) return;
    try {
      await navigator.clipboard?.writeText(token);
      setExternalFormStatus("Invite token copied.");
    } catch {
      setExternalFormStatus("Copy failed. Select the token manually.");
    }
  };

  const handleCopyGateway = async (invite) => {
    const url = gatewayUrl(settings.apiBase, invite?.token);
    if (!url) return;
    try {
      await navigator.clipboard?.writeText(url);
      setExternalFormStatus("Hermes gateway URL copied.");
    } catch {
      setExternalFormStatus("Copy failed. Select the gateway URL manually.");
    }
  };

  const handleRevokeInvite = async (invite) => {
    if (!invite?.token) return;
    try {
      await collab.revokeInvite(invite.token);
      if (inviteResult?.token === invite.token) setInviteResult(null);
      setExternalFormStatus("Invite revoked.");
    } catch (err) {
      setExternalFormStatus(`Revoke failed: ${err.message}`);
    }
  };

  const handleMcpToggle = async (server) => {
    setMcpActionStatus("");
    if (isAirgapped) {
      setMcpActionStatus("Airgapped policy blocks MCP server changes.");
      return;
    }
    try {
      await collab.updateMcpServer(server.id || server.name, { enabled: !server.enabled });
    } catch (err) {
      setMcpActionStatus(`MCP update failed: ${err.message}`);
    }
  };

  const handleMcpDelete = async (server) => {
    setMcpActionStatus("");
    if (isAirgapped) {
      setMcpActionStatus("Airgapped policy blocks MCP server changes.");
      return;
    }
    const serverId = server.id || server.name;
    try {
      await collab.deleteMcpServer(serverId);
      setExpandedMcpServerId((current) => (current === serverId ? null : current));
      setMcpToolsByServer((value) => {
        const next = { ...value };
        delete next[serverId];
        return next;
      });
    } catch (err) {
      setMcpActionStatus(`MCP delete failed: ${err.message}`);
    }
  };

  const handleMcpHealth = async (server) => {
    setMcpActionStatus("");
    try {
      const result = await collab.checkMcpHealth(server.id || server.name);
      setMcpActionStatus(`${server.name}: ${result.healthy ? "healthy" : "unhealthy"}`);
    } catch (err) {
      setMcpActionStatus(`MCP health failed: ${err.message}`);
    }
  };

  const handleMcpTools = async (server) => {
    const serverId = server.id || server.name;
    setExpandedMcpServerId((current) => (current === serverId ? null : serverId));
    if (mcpToolsByServer[serverId]?.status === "loaded") return;
    setMcpToolsByServer((value) => ({
      ...value,
      [serverId]: { status: "loading", tools: [], error: "" },
    }));
    try {
      const result = await collab.listMcpTools(serverId);
      const tools = Array.isArray(result?.tools)
        ? result.tools
        : Array.isArray(result)
          ? result
          : [];
      setMcpToolsByServer((value) => ({
        ...value,
        [serverId]: { status: "loaded", tools, error: "" },
      }));
    } catch (err) {
      setMcpToolsByServer((value) => ({
        ...value,
        [serverId]: { status: "error", tools: [], error: err.message },
      }));
    }
  };

  // ─── Mode change ───

  const handleModeChange = useCallback(async (mode) => {
    if (collab.activeProject?.id) {
      try {
        await collab.fetchJson(`/api/projects/${collab.activeProject.id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ collaboration_mode: mode }),
        });
        await collab.refreshAll();
      } catch { /* ignore */ }
    }
  }, [collab]);

  // ─── Render ───

  const wsLabel = collab.wsState === "connected" ? "Connected" : "Offline";

  return (
    <div className="collab-workspace" id="collaboration-workspace">
      {/* Header */}
      <ProjectHeader
        project={collab.activeProject}
        projects={collab.projects}
        onSelectProject={collab.setActiveProject}
        onCreateProject={collab.createProject}
        onModeChange={handleModeChange}
        localCount={collab.localAgents.length}
        externalCount={connectedExternalCount}
        activeCount={collab.activeSessions.length}
        wsState={collab.wsState}
      />

      {/* Main Canvas */}
      <div className="collab-canvas">
        {/* Left Lane: Local Agents */}
        <AgentLane
          title="Local Systems"
          subtitle="Workers, tools, and this PC"
          side="left"
          agents={collab.localAgents}
          mcpServers={[]}
          runningAgentIds={collab.runningAgentIds}
          selectedAgentId={selectedAgentId}
          onSelectAgent={setSelectedAgentId}
          onAddAgent={() => setAgentBuilderOpen(true)}
          onDeleteAgent={handleDeleteAgent}
          emptyLabel="No local agents configured."
          emptyDesc="Add your first agent to begin."
        />

        {/* Center: Workspace Visualization */}
        <div className="collab-center">
          <ProjectSphere
            agent={collab.agent1Agent}
            onClick={() => setAgent1ConfigOpen(!agent1ConfigOpen)}
          />
          <SharedWorkspaceVisual
            localAgentCount={collab.localAgents.length}
            connectedExternalCount={connectedExternalCount}
            activeSessionCount={collab.activeSessions.length}
          />
        </div>

        {/* Right Lane: External Agents / MCP Servers */}
        <AgentLane
          title="External Agents"
          subtitle="Invited agents and MCP tools"
          side="right"
          agents={collab.externals}
          mcpServers={collab.mcpServers}
          runningAgentIds={collab.runningAgentIds}
          selectedAgentId={selectedAgentId}
          onSelectAgent={setSelectedAgentId}
          onAddAgent={() => setExternalBuilderOpen(true)}
          emptyLabel="No external agents."
          emptyDesc="Invite agents to collaborate."
        />
      </div>

      {/* Bottom: Composer + Activity */}
      <div className="collab-bottom">
        <div className="collab-bottom-left">
          <StreamOutput output={collab.streamingOutput} />
          <ActivityFeed
            events={collab.recentEvents}
            pendingApprovals={collab.pendingApprovals}
            onApprove={collab.approveAction}
          />
        </div>
        <div className="collab-bottom-right">
          <TaskComposer
            agent1Agent={collab.agent1Agent}
            onSubmit={handleRunTask}
            isRunning={isRunning}
            conversation={conversation}
            onOpenConfig={() => setAgent1ConfigOpen(!agent1ConfigOpen)}
          />
        </div>
      </div>

      {/* Agent1 Config Popover */}
      {agent1ConfigOpen && (
        <div className="collab-popover glass-panel" id="agent1-config" role="dialog">
          <form onSubmit={handleSaveAgent1}>
            <div className="popover-head">
              <div>
                <strong>Agent1 Configuration</strong>
                <span className="popover-sub">Central orchestrator model settings</span>
              </div>
              <button type="button" className="btn-ghost" onClick={() => setAgent1ConfigOpen(false)}>✕</button>
            </div>
            <div className="popover-grid">
              <label>
                <span>Provider</span>
                <select
                  value={agent1Form.provider}
                  onChange={(e) => {
                    const provider = e.target.value;
                    const nextProvider = providerOptions.find((item) => item.provider === provider);
                    setAgent1Form({
                      provider,
                      model: modelName(nextProvider?.models?.[0]),
                    });
                  }}
                >
                  {providerOptions.map((provider) => (
                    <option key={provider.provider} value={provider.provider}>
                      {provider.label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>Model</span>
                <select
                  value={agent1Form.model}
                  onChange={(e) => setAgent1Form((f) => ({ ...f, model: e.target.value }))}
                  disabled={!agent1ModelOptions.length}
                >
                  {!agent1ModelOptions.length && <option value="">No models loaded</option>}
                  {agent1ModelOptions.map((model) => {
                    const name = modelName(model);
                    return (
                      <option key={`${selectedAgent1Provider?.provider}-${name}`} value={name}>
                        {name}
                      </option>
                    );
                  })}
                </select>
                {agent1ModelProviderError && (
                  <span className="field-hint error">{agent1ModelProviderError}</span>
                )}
              </label>
            </div>
            {agent1FormStatus && (
              <div className={`popover-status ${agent1FormStatus.startsWith("Error") ? "error" : "success"}`}>
                {agent1FormStatus}
              </div>
            )}
            <div className="popover-actions">
              <button type="submit" className="btn-confirm">Save Agent1</button>
            </div>
          </form>
        </div>
      )}

      {/* Agent Builder Popover */}
      {agentBuilderOpen && (
        <div className="collab-popover glass-panel" id="agent-builder" role="dialog">
          <form onSubmit={handleSaveAgent}>
            <div className="popover-head">
              <strong>Add Local Agent</strong>
              <button type="button" className="btn-ghost" onClick={() => setAgentBuilderOpen(false)}>✕</button>
            </div>
            <div className="popover-grid">
              <label>
                <span>Name</span>
                <input
                  type="text"
                  value={agentForm.name}
                  onChange={(e) => setAgentForm((f) => ({ ...f, name: e.target.value }))}
                  placeholder="Researcher"
                  autoFocus
                />
              </label>
              <label>
                <span>Role</span>
                <input
                  type="text"
                  value={agentForm.role}
                  onChange={(e) => setAgentForm((f) => ({ ...f, role: e.target.value }))}
                  placeholder="Worker, Critic, Planner..."
                />
              </label>
              <label>
                <span>Provider</span>
                <select
                  value={agentForm.provider}
                  onChange={(e) => {
                    const provider = e.target.value;
                    const nextProvider = providerOptions.find((item) => item.provider === provider);
                    setAgentForm((f) => ({
                      ...f,
                      provider,
                      model: modelName(nextProvider?.models?.[0]) || f.model,
                    }));
                  }}
                >
                  {providerOptions.map((provider) => (
                    <option key={provider.provider} value={provider.provider}>
                      {provider.label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>Model</span>
                <select
                  value={agentForm.model}
                  onChange={(e) => setAgentForm((f) => ({ ...f, model: e.target.value }))}
                  disabled={!agentModelOptions.length}
                >
                  {!agentModelOptions.length && <option value="">No models loaded</option>}
                  {agentModelOptions.map((model) => {
                    const name = modelName(model);
                    return (
                      <option key={`${selectedAgentProvider?.provider}-${name}`} value={name}>
                        {name}
                      </option>
                    );
                  })}
                </select>
                {agentModelProviderError && (
                  <span className="field-hint error">{agentModelProviderError}</span>
                )}
              </label>
            </div>
            <label>
              <span>System Prompt</span>
              <textarea
                value={agentForm.systemPrompt}
                onChange={(e) => setAgentForm((f) => ({ ...f, systemPrompt: e.target.value }))}
                rows={3}
              />
            </label>
            {agentFormStatus && <div className="popover-status">{agentFormStatus}</div>}
            <div className="popover-actions">
              <button type="submit" className="btn-confirm">Create Agent</button>
            </div>
          </form>
        </div>
      )}

      {/* External Server Builder Popover */}
      {externalBuilderOpen && (
        <div className="collab-popover glass-panel" id="external-builder" role="dialog">
          <form onSubmit={handleSaveExternal}>
            <div className="popover-head">
              <div>
                <strong>External Collaboration</strong>
                <span className="popover-sub">Invite agents or connect MCP tools</span>
              </div>
              <button type="button" className="btn-ghost" onClick={() => setExternalBuilderOpen(false)}>✕</button>
            </div>
            <div className="external-builder-section">
              <div className="collab-section-label">INVITE PERMISSIONS</div>
              {isAirgapped && (
                <div className="policy-block-note">Airgapped mode keeps external invites disabled for this project.</div>
              )}
              <div className="permission-grid">
                {[
                  ["can_read_blackboard", "Read blackboard"],
                  ["can_write_blackboard", "Write blackboard"],
                  ["can_create_artifacts", "Create artifacts"],
                  ["can_delegate_tasks", "Delegate tasks"],
                ].map(([key, label]) => (
                  <label key={key} className="permission-toggle">
                    <input
                      type="checkbox"
                      checked={Boolean(invitePermissions[key])}
                      onChange={(e) => setInvitePermissions((value) => ({ ...value, [key]: e.target.checked }))}
                    />
                    <span>{label}</span>
                  </label>
                ))}
                <label>
                  <span>Max concurrent tasks</span>
                  <input
                    type="number"
                    min="1"
                    max="16"
                    value={invitePermissions.max_concurrent_tasks}
                    onChange={(e) => setInvitePermissions((value) => ({ ...value, max_concurrent_tasks: e.target.value }))}
                  />
                </label>
                <label>
                  <span>Allowed tools</span>
                  <input
                    type="text"
                    value={invitePermissions.allowed_tools}
                    onChange={(e) => setInvitePermissions((value) => ({ ...value, allowed_tools: e.target.value }))}
                    placeholder="tool_a, tool_b"
                  />
                </label>
              </div>
              <div className="popover-actions inline">
                <button type="button" className="btn-confirm" onClick={handleCreateInvite} disabled={!collab.activeProject || isAirgapped}>
                  Generate Invite
                </button>
              </div>
              {inviteResult && (
                <div className="invite-result">
                  <span>Token</span>
                  <code>{inviteResult.token}</code>
                </div>
              )}
              <div className="hermes-connect-card">
                <div className="collab-section-label">HERMES AGENT CONNECT</div>
                <p>Local and external Hermes agents connect over Agent1's project-scoped gateway with the invite token above.</p>
                <div className="hermes-command-grid">
                  <span>Local</span>
                  <code>{hermesInvite?.token ? gatewayUrl(settings.apiBase, hermesInvite.token, "local-hermes") : "Generate an invite to create a local Hermes URL"}</code>
                  <span>External</span>
                  <code>{hermesInvite?.token ? gatewayUrl(settings.apiBase, hermesInvite.token, "external-hermes") : "Generate an invite to create an external Hermes URL"}</code>
                </div>
              </div>
              <div className="invite-manager">
                {(collab.inviteTokens || []).length > 0 ? (
                  (collab.inviteTokens || []).map((invite) => (
                    <article key={invite.token} className="invite-manager-row">
                      <div>
                        <strong>{invite.used_by ? "Used invite" : "Open invite"}</strong>
                        <code>{invite.token}</code>
                        <span>{invite.created_by || "user"} - {invite.used_by || "unused"}</span>
                      </div>
                      <div className="invite-manager-actions">
                        <button type="button" className="btn-ghost" onClick={() => handleCopyInvite(invite)}>Copy</button>
                        <button type="button" className="btn-ghost" onClick={() => handleCopyGateway(invite)}>Gateway</button>
                        <button type="button" className="btn-danger" onClick={() => handleRevokeInvite(invite)}>Revoke</button>
                      </div>
                    </article>
                  ))
                ) : (
                  <div className="invite-manager-empty">No active invites for this project.</div>
                )}
              </div>
            </div>
            <div className="external-builder-section">
              <div className="collab-section-label">MCP SERVER</div>
              {isAirgapped && (
                <div className="policy-block-note">Airgapped mode blocks MCP server changes from this workspace.</div>
              )}
            <div className="popover-grid">
              <label>
                <span>Name</span>
                <input
                  type="text"
                  value={externalForm.name}
                  onChange={(e) => setExternalForm((f) => ({ ...f, name: e.target.value }))}
                  placeholder="filesystem"
                  autoFocus
                />
              </label>
              <label>
                <span>Command</span>
                <input
                  type="text"
                  value={externalForm.command}
                  onChange={(e) => setExternalForm((f) => ({ ...f, command: e.target.value }))}
                  placeholder="npx @modelcontextprotocol/server-filesystem"
                />
              </label>
              <label>
                <span>Args (comma-separated)</span>
                <input
                  type="text"
                  value={externalForm.args}
                  onChange={(e) => setExternalForm((f) => ({ ...f, args: e.target.value }))}
                  placeholder="--path, /workspace"
                />
              </label>
            </div>
            </div>
            {externalFormStatus && <div className="popover-status">{externalFormStatus}</div>}
            <div className="popover-actions">
              <button type="submit" className="btn-confirm" disabled={isAirgapped}>Add Server</button>
            </div>
            {(collab.mcpServers || []).length > 0 && (
              <div className="external-builder-section">
                <div className="collab-section-label">MCP MANAGER</div>
                <div className="mcp-manager-list">
                  {(collab.mcpServers || []).map((server) => {
                    const serverId = server.id || server.name;
                    const toolState = mcpToolsByServer[serverId] || { status: "idle", tools: [], error: "" };
                    const isExpanded = expandedMcpServerId === serverId;
                    return (
                      <div key={serverId} className="mcp-manager-item">
                        <div className="mcp-manager-row">
                          <div>
                            <strong>{server.name}</strong>
                            <span>{server.enabled ? "enabled" : "disabled"} - {server.command || "stdio"}</span>
                          </div>
                          <div className="mcp-manager-actions">
                            <button type="button" className="btn-ghost" onClick={() => handleMcpHealth(server)}>Health</button>
                            <button type="button" className="btn-ghost" onClick={() => handleMcpTools(server)}>
                              {isExpanded ? "Hide Tools" : "Tools"}
                            </button>
                            <button type="button" className="btn-ghost" onClick={() => handleMcpToggle(server)}>
                              {server.enabled ? "Disable" : "Enable"}
                            </button>
                            <button type="button" className="btn-danger" onClick={() => handleMcpDelete(server)}>Delete</button>
                          </div>
                        </div>
                        {isExpanded && (
                          <div className="mcp-tools-drawer">
                            {toolState.status === "loading" && <div className="mcp-tools-empty">Loading tools...</div>}
                            {toolState.status === "error" && (
                              <div className="mcp-tools-empty error">{toolState.error || "Could not load tools."}</div>
                            )}
                            {toolState.status === "loaded" && toolState.tools.length === 0 && (
                              <div className="mcp-tools-empty">No tools reported by this server.</div>
                            )}
                            {toolState.status === "loaded" && toolState.tools.length > 0 && (
                              <div className="mcp-tools-list">
                                {toolState.tools.map((tool, index) => (
                                  <article key={tool.name || index} className="mcp-tool-card">
                                    <strong>{tool.name || `tool_${index + 1}`}</strong>
                                    <p>{tool.description || "No description provided."}</p>
                                  </article>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
                {mcpActionStatus && <div className="popover-status">{mcpActionStatus}</div>}
              </div>
            )}
          </form>
        </div>
      )}

      {deleteCandidate && (
        <div className="collab-modal-backdrop" role="presentation">
          <div
            className="delete-agent-dialog glass-panel"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="delete-agent-title"
            aria-describedby="delete-agent-desc"
          >
            <div className="delete-dialog-accent" aria-hidden="true" />
            <div className="delete-dialog-head">
              <span className="delete-dialog-icon" aria-hidden="true">DEL</span>
              <div>
                <span className="popover-sub">Local agent removal</span>
                <strong id="delete-agent-title">Delete agent?</strong>
              </div>
            </div>
            <p id="delete-agent-desc" className="delete-dialog-copy">
              This removes the local agent profile from Agent1. Existing activity history stays in the workspace log.
            </p>
            <div className="delete-agent-preview">
              <span className="delete-agent-avatar">
                {(deleteCandidate.name || deleteCandidate.id || "?").slice(0, 1).toUpperCase()}
              </span>
              <div>
                <strong>{deleteCandidate.name || deleteCandidate.id}</strong>
                <span>{deleteCandidate.model?.provider || deleteCandidate.role || "local agent"}</span>
              </div>
            </div>
            <div className="delete-dialog-actions">
              <button
                type="button"
                className="btn-ghost"
                onClick={() => setDeleteCandidate(null)}
                disabled={Boolean(deletingAgentId)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn-danger"
                onClick={confirmDeleteAgent}
                disabled={Boolean(deletingAgentId)}
              >
                {deletingAgentId ? "Deleting..." : "Delete Agent"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Status bar */}
      <div className="collab-status-bar">
        <span>
          {collab.activeSessions.length > 0
            ? `${collab.activeSessions.length} session(s) running`
            : wsLabel}
        </span>
        <button type="button" className="status-inspector-btn" onClick={() => setInspectorOpen(true)}>
          Inspect
        </button>
      </div>

      {inspectorOpen && (
        <ProjectInspector collab={collab} apiBase={settings.apiBase} onClose={() => setInspectorOpen(false)} />
      )}
    </div>
  );
}
