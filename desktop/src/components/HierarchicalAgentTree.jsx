import { useState, useMemo } from "react";

function getAgentInitials(name = "") {
  return name
    .split(" ")
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
}

function getRoleColor(role) {
  if (!role) return "var(--text-mono)";
  const r = role.toLowerCase();
  if (r.includes("orchestrator")) return "var(--accent-purple)";
  if (r.includes("planner")) return "var(--accent-secondary)";
  if (r.includes("worker")) return "var(--accent-primary)";
  if (r.includes("critic")) return "var(--accent-warning)";
  if (r.includes("researcher")) return "var(--accent-secondary)";
  if (r.includes("builder")) return "var(--accent-primary)";
  if (r.includes("reporter")) return "var(--text-secondary)";
  return "var(--text-mono)";
}

function getServerColor(name = "") {
  const n = name.toLowerCase();
  if (n.includes("filesystem") || n.includes("file")) return "var(--accent-secondary)";
  if (n.includes("git")) return "var(--accent-secondary)";
  if (n.includes("slack")) return "var(--accent-purple)";
  if (n.includes("web")) return "var(--accent-primary)";
  return "var(--text-secondary)";
}

function AgentNode({ agent, depth = 0, isRunning, runningAgentIds, onSelect, selectedAgentId, expandedNodes, onToggleExpand, side }) {
  const isExpanded = expandedNodes.has(agent.id);
  const hasTools = agent.tools?.length > 0;
  const childCount = hasTools ? agent.tools.length : 0;

  const role = agent.role || agent.model?.provider || "";
  const roleColor = getRoleColor(role);

  const handleClick = () => {
    onSelect(agent.id);
  };

  const handleToggle = (e) => {
    e.stopPropagation();
    onToggleExpand(agent.id);
  };

  return (
    <div className={`hive-node hive-node-${side}`} style={{ "--depth": depth }}>
      <div
        className={`hive-node-card ${selectedAgentId === agent.id ? "selected" : ""} ${isRunning ? "running" : ""}`}
        onClick={handleClick}
      >
        <div
          className="hive-avatar"
          style={{
            background: isRunning
              ? "linear-gradient(135deg, rgba(255,179,71,0.2), rgba(255,179,71,0.1))"
              : "linear-gradient(135deg, rgba(44,224,163,0.2), rgba(44,224,163,0.1))",
            borderColor: isRunning ? "rgba(255,179,71,0.5)" : "rgba(44,224,163,0.4)",
            color: isRunning ? "var(--accent-warning)" : roleColor,
          }}
        >
          {getAgentInitials(agent.name)}
        </div>
        <div className="hive-node-info">
          <div className="hive-node-name">{agent.name}</div>
          <div className="hive-node-meta" style={{ color: roleColor }}>
            {role || agent.model?.model || "agent"}
          </div>
        </div>
        {hasTools && (
          <div className="hive-toggle" onClick={handleToggle}>
            {isExpanded ? "-" : `+${childCount}`}
          </div>
        )}
        <div className={`hive-status-dot ${isRunning ? "running" : "idle"}`} />
      </div>

      {isExpanded && hasTools && (
        <div className="hive-children">
          {(agent.tools || []).map((tool, i) => (
            <div key={i} className="hive-tool">
              <span className="hive-tool-arrow">-&gt;</span>
              <span className="hive-tool-name">{tool}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function McpNode({ server, depth = 0, expandedNodes, onToggleExpand, side }) {
  const isExpanded = expandedNodes.has(server.id || server.name);
  const color = getServerColor(server.name);
  const toolCount = server.tools?.length || 0;

  return (
    <div className={`hive-node hive-node-${side}`} style={{ "--depth": depth }}>
      <div
        className="hive-node-card mcp"
        style={{ borderColor: `${color}40` }}
      >
        <div
          className="hive-avatar"
          style={{
            background: `linear-gradient(135deg, ${color}20, ${color}08)`,
            borderColor: `${color}50`,
            color: color,
          }}
        >
          {getAgentInitials(server.name)}
        </div>
        <div className="hive-node-info">
          <div className="hive-node-name">{server.name}</div>
          <div className="hive-node-meta" style={{ color }}>
            {server.enabled ? "enabled" : "disabled"} / {toolCount} tools
          </div>
        </div>
        {toolCount > 0 && (
          <div className="hive-toggle" onClick={() => onToggleExpand(server.id || server.name)}>
            {isExpanded ? "-" : `+${toolCount}`}
          </div>
        )}
        <div className={`hive-status-dot ${server.enabled ? "active" : "idle"}`} />
      </div>

      {isExpanded && toolCount > 0 && (
        <div className="hive-children">
          {(server.tools || []).map((tool, i) => (
            <div key={i} className="hive-tool">
              <span className="hive-tool-arrow">-&gt;</span>
              <span className="hive-tool-name">{tool}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function HierarchicalAgentTree({
  agents,
  mcpServers,
  runningAgentIds,
  selectedAgentId,
  onSelectAgent,
  side,
}) {
  const [expandedNodes, setExpandedNodes] = useState(new Set());

  const toggleExpand = (id) => {
    setExpandedNodes((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const sortedAgents = useMemo(() => {
    return [...(agents || [])].sort((a, b) => {
      const aRunning = runningAgentIds.has(a.id);
      const bRunning = runningAgentIds.has(b.id);
      if (aRunning && !bRunning) return -1;
      if (!aRunning && bRunning) return 1;
      return 0;
    });
  }, [agents, runningAgentIds]);

  const sortedMcp = useMemo(() => {
    return [...(mcpServers || [])].sort((a, b) => {
      if (a.enabled && !b.enabled) return -1;
      if (!a.enabled && b.enabled) return 1;
      return 0;
    });
  }, [mcpServers]);

  const isLeft = side === "left";

  const renderTree = () => {
    if (side === "left") {
      if (sortedAgents.length === 0) {
        return (
          <div className="hive-empty">
            <span className="empty-icon">AI</span>
            <strong>No local agents configured yet.</strong>
            <span>Create your first agent to begin orchestration.</span>
          </div>
        );
      }

      const tiers = {
        orchestrator: sortedAgents.filter(a => a.role?.toLowerCase().includes("orchestrator")),
        specialized: sortedAgents.filter(a => a.role && !a.role.toLowerCase().includes("orchestrator")),
        general: sortedAgents.filter(a => !a.role),
      };

      return (
        <div className="hive-tree">
          {tiers.orchestrator.length > 0 && (
            <div className="hive-tier">
              <div className="hive-tier-label">Orchestrator</div>
              {tiers.orchestrator.map((agent, i) => (
                <AgentNode
                  key={agent.id}
                  agent={agent}
                  depth={0}
                  isRunning={runningAgentIds.has(agent.id)}
                  runningAgentIds={runningAgentIds}
                  onSelect={onSelectAgent}
                  selectedAgentId={selectedAgentId}
                  expandedNodes={expandedNodes}
                  onToggleExpand={toggleExpand}
                  side={side}
                />
              ))}
            </div>
          )}
          {tiers.specialized.length > 0 && (
            <div className="hive-tier">
              <div className="hive-tier-label">Specialized</div>
              {tiers.specialized.map((agent, i) => (
                <AgentNode
                  key={agent.id}
                  agent={agent}
                  depth={0}
                  isRunning={runningAgentIds.has(agent.id)}
                  runningAgentIds={runningAgentIds}
                  onSelect={onSelectAgent}
                  selectedAgentId={selectedAgentId}
                  expandedNodes={expandedNodes}
                  onToggleExpand={toggleExpand}
                  side={side}
                />
              ))}
            </div>
          )}
          {tiers.general.length > 0 && (
            <div className="hive-tier">
              <div className="hive-tier-label">General</div>
              {tiers.general.map((agent, i) => (
                <AgentNode
                  key={agent.id}
                  agent={agent}
                  depth={0}
                  isRunning={runningAgentIds.has(agent.id)}
                  runningAgentIds={runningAgentIds}
                  onSelect={onSelectAgent}
                  selectedAgentId={selectedAgentId}
                  expandedNodes={expandedNodes}
                  onToggleExpand={toggleExpand}
                  side={side}
                />
              ))}
            </div>
          )}
        </div>
      );
    } else {
      if (sortedMcp.length === 0) {
        return (
          <div className="hive-empty">
            <span className="empty-icon">MCP</span>
            <strong>No external servers connected.</strong>
            <span>Add MCP or external tools to expand the canvas.</span>
          </div>
        );
      }

      const enabled = sortedMcp.filter(s => s.enabled);
      const disabled = sortedMcp.filter(s => !s.enabled);

      return (
        <div className="hive-tree">
          {enabled.length > 0 && (
            <div className="hive-tier">
              <div className="hive-tier-label">Active</div>
              {enabled.map((server, i) => (
                <McpNode
                  key={server.id || server.name}
                  server={server}
                  depth={0}
                  expandedNodes={expandedNodes}
                  onToggleExpand={toggleExpand}
                  side={side}
                />
              ))}
            </div>
          )}
          {disabled.length > 0 && (
            <div className="hive-tier">
              <div className="hive-tier-label">Inactive</div>
              {disabled.map((server, i) => (
                <McpNode
                  key={server.id || server.name}
                  server={server}
                  depth={0}
                  expandedNodes={expandedNodes}
                  onToggleExpand={toggleExpand}
                  side={side}
                />
              ))}
            </div>
          )}
        </div>
      );
    }
  };

  return (
    <div className={`hive-tree-container hive-tree-${side}`}>
      <div className="hive-header">
        {side === "left" ? "Local Agents" : "External Agents"}
      </div>
      {renderTree()}
    </div>
  );
}
