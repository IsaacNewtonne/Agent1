import ProjectSphere from "./ProjectSphere";
import HierarchicalAgentTree from "./HierarchicalAgentTree";
import SharedWorkspaceVisual from "./SharedWorkspaceVisual";

export default function CanvasGraph({
  agents,
  mcpServers,
  runningAgentIds,
  selectedAgentId,
  onSelectAgent,
  agent1Agent,
  onOpenAgent1Config,
  localAgentCount,
  externalServerCount,
  connectedExternalCount,
  activeSessionCount,
  wsLabel,
  wsState,
}) {
  return (
    <section className="canvas-graph" aria-label="Agent orchestration canvas">
      <div className="canvas-graph-topline">
        <div className="canvas-mini-status">
          <span className="metric-chip connected"><strong>{agent1Agent ? "Ready" : "Setup"}</strong> Agent1</span>
          <span className="metric-chip"><strong>{localAgentCount}</strong> Local</span>
          <span className="metric-chip"><strong>{connectedExternalCount}/{externalServerCount}</strong> External</span>
          <span className={`metric-chip state ${wsState === "connected" ? "connected" : "offline"}`}>
            <span className={`status-dot ${wsState === "connected" ? "connected" : "disconnected"}`} />
            {activeSessionCount > 0 ? `${activeSessionCount} running` : wsLabel}
          </span>
        </div>
      </div>

      <div className="canvas-graph-field">
        <div className="graph-lane graph-lane-local">
          <div className="lane-heading">
            <span className="lane-icon">LOC</span>
            <div>
              <h2>Local Systems</h2>
              <p>Workers, tools, and this PC</p>
            </div>
          </div>
          <HierarchicalAgentTree
            agents={agents}
            mcpServers={mcpServers}
            runningAgentIds={runningAgentIds}
            selectedAgentId={selectedAgentId}
            onSelectAgent={onSelectAgent}
            side="left"
          />
        </div>

        <div className="graph-core">
          <ProjectSphere
            agent={agent1Agent}
            onClick={onOpenAgent1Config}
          />
        </div>

        <div className="graph-workspace">
          <SharedWorkspaceVisual
            localAgentCount={localAgentCount}
            connectedExternalCount={connectedExternalCount}
            activeSessionCount={activeSessionCount}
          />
        </div>

        <div className="graph-lane graph-lane-external">
          <div className="lane-heading">
            <span className="lane-icon">EXT</span>
            <div>
              <h2>External Agents</h2>
              <p>Invited and permissioned</p>
            </div>
          </div>
          <HierarchicalAgentTree
            agents={agents}
            mcpServers={mcpServers}
            runningAgentIds={runningAgentIds}
            selectedAgentId={selectedAgentId}
            onSelectAgent={onSelectAgent}
            side="right"
          />
        </div>
      </div>

      <div className="canvas-graph-spacer" aria-hidden="true" />
    </section>
  );
}
