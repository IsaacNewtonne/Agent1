import { memo } from "react";

const ProjectSphere = memo(function ProjectSphere({ agent, onClick }) {
  const modelLabel = agent?.model?.provider
    ? `${agent.model.provider}/${agent.model.model}`
    : "Configure model";

  return (
    <div className="project-sphere-container">
      <button
        type="button"
        className="agent1-core-button"
        onClick={onClick}
        aria-label="Open Agent1 configuration"
      >
        <span className="agent1-rainbow-ring" aria-hidden="true" />
        <span className="agent1-logo-shell">
          <img src="/icons/agent1-logo.png" alt="" />
        </span>
      </button>
      <span className="agent1-model-tooltip" role="tooltip">
        {modelLabel}
      </span>
    </div>
  );
});

export default ProjectSphere;
