import { memo, useCallback, useState, useMemo, useRef, useEffect } from "react";
import { createPortal } from "react-dom";

// ─── Mode Selector ───

export const ModeSelector = memo(function ModeSelector({ mode, onChange }) {
  const modes = [
    { value: "automatic", label: "Automatic", desc: "Agent1 decides", icon: "⚡" },
    { value: "structured", label: "Structured", desc: "Plan → Execute → Review", icon: "📋" },
    { value: "fast", label: "Fast", desc: "Parallel, minimal oversight", icon: "🚀" },
    { value: "careful", label: "Careful", desc: "Approval required", icon: "🛡" },
  ];

  return (
    <div className="collab-mode-selector" role="radiogroup" aria-label="Collaboration mode">
      {modes.map((m) => (
        <button
          key={m.value}
          type="button"
          className={`mode-option ${mode === m.value ? "active" : ""}`}
          onClick={() => onChange(m.value)}
          role="radio"
          aria-checked={mode === m.value}
          aria-describedby={`mode-tip-${m.value}`}
        >
          <span className="mode-icon">{m.icon}</span>
          <span className="mode-label">{m.label}</span>
          <span className="mode-desc" id={`mode-tip-${m.value}`} role="tooltip">
            {m.desc}
          </span>
        </button>
      ))}
    </div>
  );
});

// ─── Project Header ───

export const ProjectHeader = memo(function ProjectHeader({
  project,
  projects,
  onSelectProject,
  onCreateProject,
  onModeChange,
  localCount,
  externalCount,
  activeCount,
  wsState,
}) {
  const [createOpen, setCreateOpen] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createMode, setCreateMode] = useState("automatic");

  const handleCreate = async (e) => {
    e.preventDefault();
    if (!createName.trim()) return;
    await onCreateProject(createName.trim(), createMode);
    setCreateName("");
    setCreateOpen(false);
  };
  const createProjectPopover = createOpen ? (
    <div className="create-project-layer" role="presentation">
      <div className="create-project-popover glass-panel" role="dialog" aria-label="Create project">
        <form onSubmit={handleCreate}>
          <strong>New Project</strong>
          <input
            type="text"
            value={createName}
            onChange={(e) => setCreateName(e.target.value)}
            placeholder="Project name..."
            autoFocus
            id="create-project-name"
          />
          <ModeSelector mode={createMode} onChange={setCreateMode} />
          <div className="create-project-actions">
            <button type="submit" className="btn-confirm" disabled={!createName.trim()}>
              Create
            </button>
            <button type="button" className="btn-ghost" onClick={() => setCreateOpen(false)}>
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  ) : null;

  return (
    <>
      <header className="collab-header" id="project-header">
        <div className="collab-header-left">
          <div className="collab-brand">
            <img src="/icons/agent1-logo.png" alt="" className="collab-brand-logo" />
            <div>
              <h1 className="collab-brand-title">Agent1</h1>
              <span className="collab-brand-sub">Hybrid Workspace</span>
            </div>
          </div>
        </div>

        <div className="collab-header-center">
          {project ? (
            <div className="collab-project-badge">
              <span className="project-glow" aria-hidden="true" />
              <label className="project-switcher">
                <span className="sr-only">Active project</span>
                <select
                  value={project.id || ""}
                  onChange={(e) => {
                    const nextProject = (projects || []).find((p) => p.id === e.target.value);
                    if (nextProject) onSelectProject(nextProject);
                  }}
                  aria-label="Select project"
                >
                  {(projects || []).map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
              <ModeSelector
                mode={project.collaboration_mode || "automatic"}
                onChange={onModeChange}
              />
            </div>
          ) : (
            <button
              type="button"
              className="btn-create-project"
              onClick={() => setCreateOpen(true)}
              id="create-project-btn"
            >
              <span>+</span> Create Project
            </button>
          )}
        </div>

        <div className="collab-header-right">
          <div className="collab-metrics">
            <span className="metric-chip connected">
              <span className={`status-dot ${wsState === "connected" ? "connected" : "disconnected"}`} />
              {wsState === "connected" ? "Live" : "Offline"}
            </span>
            <span className="metric-chip"><strong>{localCount}</strong> Local</span>
            <span className="metric-chip"><strong>{externalCount}</strong> External</span>
            {activeCount > 0 && (
              <span className="metric-chip running"><strong>{activeCount}</strong> Active</span>
            )}
          </div>
        </div>
      </header>
      {createProjectPopover ? createPortal(createProjectPopover, document.body) : null}
    </>
  );
});

// ─── Agent Card (for lanes) ───

export const AgentCard = memo(function AgentCard({
  agent,
  isRunning,
  isSelected,
  onClick,
  onDelete,
  side,
}) {
  const initials = (agent.name || "")
    .split(" ")
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);

  const role = agent.role || agent.model?.provider || "";

  return (
    <div
      className={`collab-agent-card ${side} ${isRunning ? "running" : ""} ${isSelected ? "selected" : ""}`}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
      role="button"
      tabIndex={0}
      aria-pressed={isSelected}
    >
      <div className={`collab-agent-avatar ${isRunning ? "running" : ""}`}>
        {initials}
      </div>
      <div className="collab-agent-info">
        <span className="collab-agent-name">{agent.name}</span>
        <span className="collab-agent-role">{role || "agent"}</span>
      </div>
      {onDelete && (
        <button
          type="button"
          className="collab-agent-delete"
          onClick={(event) => {
            event.stopPropagation();
            onDelete(agent);
          }}
          aria-label={`Delete ${agent.name}`}
          title={`Delete ${agent.name}`}
        >
          Delete
        </button>
      )}
      <div className={`collab-status-dot ${isRunning ? "running" : "idle"}`} />
    </div>
  );
});

// ─── External Agent Card ───

export const ExternalCard = memo(function ExternalCard({ server, side }) {
  const initials = (server.name || "")
    .split(" ")
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
  const status = server.status || (server.enabled ? "active" : "inactive");
  const capabilityCount = server.capabilities?.length ?? server.tools?.length ?? 0;
  const statusActive = status === "connected" || status === "active";

  return (
    <div className={`collab-agent-card ${side} external`}>
      <div className="collab-agent-avatar external">
        {initials}
      </div>
      <div className="collab-agent-info">
        <span className="collab-agent-name">{server.name}</span>
        <span className="collab-agent-role">
          {server.enabled ? "active" : "inactive"} · {server.tools?.length || 0} tools
        </span>
      </div>
      <div className={`collab-status-dot ${server.enabled ? "active" : "idle"}`} />
    </div>
  );
});

// ─── Lane (Local / External) ───

export const AgentLane = memo(function AgentLane({
  title,
  subtitle,
  side,
  agents,
  mcpServers,
  runningAgentIds,
  selectedAgentId,
  onSelectAgent,
  onAddAgent,
  onDeleteAgent,
  emptyLabel,
  emptyDesc,
}) {
  const isLeft = side === "left";
  const items = isLeft ? agents : mcpServers;

  return (
    <div className={`collab-lane collab-lane-${side}`}>
      <div className="collab-lane-header">
        <span className={`collab-lane-badge ${side}`}>{isLeft ? "LOC" : "EXT"}</span>
        <div>
          <h2 className="collab-lane-title">{title}</h2>
          <p className="collab-lane-sub">{subtitle}</p>
        </div>
      </div>

      <div className="collab-lane-list">
        {isLeft ? (
          agents.length > 0 ? (
            agents.map((agent) => (
              <AgentCard
                key={agent.id}
                agent={agent}
                isRunning={runningAgentIds.has(agent.id)}
                isSelected={selectedAgentId === agent.id}
                onClick={() => onSelectAgent(agent.id)}
                onDelete={onDeleteAgent}
                side={side}
              />
            ))
          ) : (
            <div className="collab-lane-empty">
              <span className="empty-icon">AI</span>
              <strong>{emptyLabel}</strong>
              <span>{emptyDesc}</span>
            </div>
          )
        ) : (
          (agents || []).length > 0 || (mcpServers || []).length > 0 ? (
            <>
              {(agents || []).map((agent) => (
                <ExternalCard
                  key={agent.id || agent.name}
                  server={{
                    ...agent,
                    enabled: agent.status === "connected",
                    tools: agent.capabilities || [],
                  }}
                  side={side}
                />
              ))}
              {(mcpServers || []).map((server) => (
                <ExternalCard
                  key={server.id || server.name}
                  server={server}
                  side={side}
                />
              ))}
            </>
          ) : (
            <div className="collab-lane-empty">
              <span className="empty-icon">MCP</span>
              <strong>{emptyLabel}</strong>
              <span>{emptyDesc}</span>
            </div>
          )
        )}
      </div>

      {onAddAgent && (
        <button
          type="button"
          className={`btn-add-lane ${side}`}
          onClick={onAddAgent}
        >
          + {isLeft ? "Add Agent" : "Invite External"}
        </button>
      )}
    </div>
  );
});

// ─── Activity Feed ───

export const ActivityFeed = memo(function ActivityFeed({ events, pendingApprovals, onApprove }) {
  const [expandedIds, setExpandedIds] = useState(new Set());

  const toggleExpand = (id) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const formatTime = (ts) => {
    if (!ts) return "";
    try {
      return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    } catch { return ""; }
  };

  return (
    <div className="collab-activity">
      {pendingApprovals.length > 0 && (
        <div className="collab-approvals">
          <div className="collab-section-label">PENDING APPROVALS</div>
          {pendingApprovals.map((approval) => (
            <div key={approval.id} className="collab-approval-card">
              <div className="approval-structured">
                <div className="approval-structured-head">
                  <span>{approval.request?.tool_name || "Action"}</span>
                  <strong>{approval.request?.risk_level || "unknown"} risk</strong>
                </div>
                <div className="approval-meta-grid">
                  <span>Agent</span>
                  <strong>{approval.agent_id || approval.request?.agent_id || "agent"}</strong>
                  <span>Approval</span>
                  <strong>{approval.id}</strong>
                </div>
                <pre>
                  {JSON.stringify(approval.request?.input || {}, null, 2).slice(0, 260)}
                </pre>
              </div>
              <div className="approval-header">
                ⚠ {approval.request?.tool_name || "Action"} — {approval.agent_id || "agent"}
              </div>
              <div className="approval-request">
                {JSON.stringify(approval.request?.input || {}, null, 2).slice(0, 200)}
              </div>
              <div className="approval-actions">
                <button
                  type="button"
                  className="btn-confirm small"
                  onClick={() => onApprove(approval.id, true)}
                >
                  Approve
                </button>
                <button
                  type="button"
                  className="btn-danger small"
                  onClick={() => onApprove(approval.id, false)}
                >
                  Deny
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="collab-section-label">ACTIVITY</div>
      <div className="collab-event-list">
        {events.slice(0, 15).map((event) => {
          const isError = event.event_type?.includes("Error") || event.event_type?.includes("Failed");
          const isExpanded = expandedIds.has(event.id);

          return (
            <div
              key={event.id}
              className={`collab-event ${isError ? "error" : ""} ${isExpanded ? "expanded" : ""}`}
              onClick={() => toggleExpand(event.id)}
              onKeyDown={(keyboardEvent) => {
                if (keyboardEvent.key === "Enter" || keyboardEvent.key === " ") {
                  keyboardEvent.preventDefault();
                  toggleExpand(event.id);
                }
              }}
              role="button"
              tabIndex={0}
              aria-expanded={isExpanded}
            >
              <div className="event-header">
                <span className={`event-type ${isError ? "error" : ""}`}>
                  {(event.event_type || "").replace(/([A-Z])/g, " $1").trim()}
                </span>
                <span className="event-time">{formatTime(event.created_at)}</span>
              </div>
              <div className="event-payload">
                {typeof event.payload === "object"
                  ? JSON.stringify(event.payload)
                  : String(event.payload || "")}
              </div>
            </div>
          );
        })}
        {events.length === 0 && (
          <div className="collab-lane-empty compact">
            <span>No activity yet. Run a task to get started.</span>
          </div>
        )}
      </div>
    </div>
  );
});

// ─── Stream Output ───

export const StreamOutput = memo(function StreamOutput({ output }) {
  const ref = useRef(null);

  useEffect(() => {
    if (ref.current) {
      ref.current.scrollTop = ref.current.scrollHeight;
    }
  }, [output]);

  return (
    <div
      ref={ref}
      className={`collab-stream ${output ? "" : "idle"}`}
      aria-live="polite"
      aria-atomic="false"
    >
      {output || "Waiting for agent output..."}
    </div>
  );
});

// ─── Composer (Task Input) ───

export const TaskComposer = memo(function TaskComposer({
  agent1Agent,
  onSubmit,
  isRunning,
  conversation,
  onOpenConfig,
}) {
  const [input, setInput] = useState("");
  const [workspace, setWorkspace] = useState(".");
  const textareaRef = useRef(null);

  const handleSubmit = (e) => {
    e.preventDefault();
    if (!input.trim() || isRunning) return;
    onSubmit(input.trim(), workspace);
    setInput("");
  };

  const handleKeyDown = (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  return (
    <div className="collab-composer">
      <form onSubmit={handleSubmit}>
        <div className="composer-left">
          <div className="composer-label-row">
            <strong>Agent1 Command</strong>
            <span className="char-count">{input.length}/4000</span>
          </div>
          <input
            type="text"
            value={workspace}
            onChange={(e) => setWorkspace(e.target.value)}
            placeholder="Workspace root"
            className="composer-workspace"
          />
        </div>

        <div className="composer-thread-area">
          {conversation.length > 0 ? (
            <div className="composer-thread">
              {conversation.slice(-4).map((msg, i) => (
                <div key={i} className={`chat-line ${msg.role}`}>
                  <span>{msg.role === "user" ? "YOU" : "A1"}</span>
                  <p>{(msg.content || "").slice(0, 300)}</p>
                </div>
              ))}
            </div>
          ) : (
            <div className="composer-thread empty">
              <div className="composer-empty">
                <span>💬</span>
                <span>Talk to Agent1 — it orchestrates everything.</span>
              </div>
            </div>
          )}
        </div>

        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Tell Agent1 what to do..."
          maxLength={4000}
          disabled={isRunning}
          id="task-input"
        />

        <div className="composer-actions">
          <button
            type="submit"
            className="btn-confirm"
            disabled={!input.trim() || isRunning || !agent1Agent}
            id="run-task-btn"
          >
            {isRunning ? "Running..." : agent1Agent ? "Run" : "Setup Agent1"}
          </button>
          <button
            type="button"
            className="btn-ghost"
            onClick={onOpenConfig}
          >
            Configure
          </button>
        </div>
      </form>
    </div>
  );
});
