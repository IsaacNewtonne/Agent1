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

export default function CollabWorkspace() {
  const settings = loadSettings();
  const collab = useCollaboration(settings.apiBase);

  // Local UI state
  const [selectedAgentId, setSelectedAgentId] = useState(null);
  const [agentBuilderOpen, setAgentBuilderOpen] = useState(false);
  const [externalBuilderOpen, setExternalBuilderOpen] = useState(false);
  const [agent1ConfigOpen, setAgent1ConfigOpen] = useState(false);
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

  // Derived
  const connectedExternalCount = useMemo(
    () => (collab.mcpServers || []).filter((s) => s.enabled).length,
    [collab.mcpServers]
  );
  const providerOptions = useMemo(() => {
    const fallback = ["opencode", "ollama", "openai_compatible"].map((provider) => ({
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
  const agent1ModelOptions = useMemo(() => {
    const options = selectedAgent1Provider?.models || [];
    if (!agent1Form.model) return options;
    if (options.some((model) => modelName(model) === agent1Form.model)) return options;
    return [{ name: agent1Form.model }, ...options];
  }, [agent1Form.model, selectedAgent1Provider]);
  const agent1ModelProviderError = selectedAgent1Provider?.error || "";

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
      const result = await collab.runTask(input, collab.agent1Agent?.id, workspace);
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
    if (!externalForm.name.trim() || !externalForm.command.trim()) {
      setExternalFormStatus("Name and command required"); return;
    }
    try {
      const args = externalForm.args ? externalForm.args.split(",").map((s) => s.trim()).filter(Boolean) : [];
      await collab.saveMcpServer({
        name: externalForm.name.trim(),
        transport: "stdio",
        command: externalForm.command.trim(),
        args,
        env: {},
        enabled: externalForm.enabled,
      });
      setExternalFormStatus("Added!");
      setTimeout(() => { setExternalFormStatus(""); setExternalBuilderOpen(false); }, 800);
      setExternalForm({ name: "", command: "", args: "", enabled: true });
    } catch (err) {
      setExternalFormStatus(`Error: ${err.message}`);
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
          subtitle="Invited and permissioned"
          side="right"
          agents={[]}
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
                <input
                  type="text"
                  value={agentForm.provider}
                  onChange={(e) => setAgentForm((f) => ({ ...f, provider: e.target.value }))}
                  placeholder="opencode"
                />
              </label>
              <label>
                <span>Model</span>
                <input
                  type="text"
                  value={agentForm.model}
                  onChange={(e) => setAgentForm((f) => ({ ...f, model: e.target.value }))}
                  placeholder="gpt-4o"
                />
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
              <strong>Invite External Agent</strong>
              <button type="button" className="btn-ghost" onClick={() => setExternalBuilderOpen(false)}>✕</button>
            </div>
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
            {externalFormStatus && <div className="popover-status">{externalFormStatus}</div>}
            <div className="popover-actions">
              <button type="submit" className="btn-confirm">Add Server</button>
            </div>
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
        {collab.activeSessions.length > 0
          ? `${collab.activeSessions.length} session(s) running`
          : wsLabel}
      </div>
    </div>
  );
}
