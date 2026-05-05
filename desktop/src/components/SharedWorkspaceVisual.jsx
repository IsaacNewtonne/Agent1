import { memo, useEffect, useMemo, useRef } from "react";

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

const SharedWorkspaceVisual = memo(function SharedWorkspaceVisual({
  localAgentCount = 0,
  connectedExternalCount = 0,
  activeSessionCount = 0,
}) {
  const canvasRef = useRef(null);
  const frameRef = useRef(null);
  const particlesRef = useRef([]);
  const streamsRef = useRef([]);
  const metrics = useMemo(() => {
    const localInfluence = clamp(localAgentCount + activeSessionCount * 2, 0, 8);
    const externalInfluence = clamp(connectedExternalCount, 0, 8);
    return {
      localInfluence,
      externalInfluence,
      totalInfluence: localInfluence + externalInfluence,
    };
  }, [localAgentCount, connectedExternalCount, activeSessionCount]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return undefined;

    const context = canvas.getContext("2d");
    let width = 0;
    let height = 0;
    let time = 0;
    let dpr = window.devicePixelRatio || 1;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      width = Math.max(1, Math.floor(rect.width));
      height = Math.max(1, Math.floor(rect.height));
      dpr = window.devicePixelRatio || 1;
      canvas.width = Math.floor(width * dpr);
      canvas.height = Math.floor(height * dpr);
      context.setTransform(dpr, 0, 0, dpr, 0, 0);

      const baseCount = 124;
      const activityCount = metrics.totalInfluence * 16;
      const nextCount = clamp(baseCount + activityCount, 124, 260);
      particlesRef.current = Array.from({ length: nextCount }, (_, index) => {
        const theta = Math.random() * Math.PI * 2;
        const phi = Math.acos(2 * Math.random() - 1);
        const shell = 0.34 + Math.random() * 0.68;
        return {
          theta,
          phi,
          shell,
          wobble: Math.random() * Math.PI * 2,
          phase: Math.random() * Math.PI * 2,
          speed: 0.003 + Math.random() * 0.006,
          size: 0.7 + Math.random() * 1.65,
          mix: Math.random(),
          index,
        };
      });
    };

    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();

    const spawnStreams = () => {
      const localChance = 0.018 + metrics.localInfluence * 0.008;
      const externalChance = 0.018 + metrics.externalInfluence * 0.008;
      if (metrics.localInfluence > 0 && Math.random() < localChance) {
        streamsRef.current.push({
          side: "left",
          progress: 0,
          lane: Math.random(),
          speed: 0.012 + Math.random() * 0.008,
          size: 1.4 + Math.random() * 1.8,
        });
      }
      if (metrics.externalInfluence > 0 && Math.random() < externalChance) {
        streamsRef.current.push({
          side: "right",
          progress: 0,
          lane: Math.random(),
          speed: 0.012 + Math.random() * 0.008,
          size: 1.4 + Math.random() * 1.8,
        });
      }
      streamsRef.current = streamsRef.current.slice(-90);
    };

    const colorForParticle = (particle, alpha) => {
      const total = Math.max(1, metrics.totalInfluence);
      const localWeight = metrics.localInfluence / total;
      const externalWeight = metrics.externalInfluence / total;
      const neutral = metrics.totalInfluence === 0 || particle.mix > clamp(metrics.totalInfluence / 12, 0, 0.78);
      if (neutral) return `rgba(244, 250, 255, ${alpha})`;
      if (particle.mix < localWeight) return `rgba(255, 113, 220, ${alpha})`;
      if (particle.mix < localWeight + externalWeight) return `rgba(88, 199, 255, ${alpha})`;
      return `rgba(244, 250, 255, ${alpha})`;
    };

    const draw = () => {
      time += 1;
      context.clearRect(0, 0, width, height);

      const centerX = width / 2;
      const centerY = height * 0.48;
      const fieldRadius = Math.min(width, height) * 0.34;
      const localGlow = clamp(metrics.localInfluence / 8, 0, 1);
      const externalGlow = clamp(metrics.externalInfluence / 8, 0, 1);

      const aura = context.createRadialGradient(centerX, centerY, 0, centerX, centerY, fieldRadius * 2.1);
      aura.addColorStop(0, `rgba(244, 250, 255, ${0.09 + metrics.totalInfluence * 0.006})`);
      aura.addColorStop(0.34, `rgba(255, 113, 220, ${0.11 + 0.06 * localGlow})`);
      aura.addColorStop(0.46, `rgba(88, 199, 255, ${0.1 + 0.05 * externalGlow})`);
      aura.addColorStop(0.72, "rgba(2, 7, 17, 0)");
      aura.addColorStop(1, "rgba(2, 7, 17, 0)");
      context.beginPath();
      context.arc(centerX, centerY, fieldRadius * 1.46, 0, Math.PI * 2);
      context.fillStyle = aura;
      context.fill();

      const rail = context.createLinearGradient(0, centerY, width, centerY);
      rail.addColorStop(0, "rgba(255, 113, 220, 0)");
      rail.addColorStop(0.28, "rgba(255, 113, 220, 0.28)");
      rail.addColorStop(0.5, "rgba(244, 250, 255, 0.3)");
      rail.addColorStop(0.72, "rgba(88, 199, 255, 0.28)");
      rail.addColorStop(1, "rgba(88, 199, 255, 0)");
      context.beginPath();
      context.moveTo(width * 0.03, centerY);
      context.lineTo(width * 0.97, centerY);
      context.strokeStyle = rail;
      context.lineWidth = 1;
      context.stroke();

      const orbitColors = [
        "rgba(244, 250, 255, 0.075)",
        "rgba(255, 113, 220, 0.085)",
        "rgba(88, 199, 255, 0.085)",
      ];
      for (let orbit = 0; orbit < 4; orbit += 1) {
        const phase = time * (0.002 + orbit * 0.0007) + orbit * 0.9;
        context.save();
        context.translate(centerX, centerY);
        context.rotate(Math.sin(phase) * 0.18 + (orbit - 1.5) * 0.28);
        context.beginPath();
        context.ellipse(0, 0, fieldRadius * (1.2 + orbit * 0.09), fieldRadius * (0.56 + orbit * 0.045), 0, 0, Math.PI * 2);
        context.strokeStyle = orbitColors[orbit % orbitColors.length];
        context.lineWidth = 1;
        context.stroke();
        context.restore();
      }

      spawnStreams();

      for (const stream of streamsRef.current) {
        stream.progress += stream.speed;
        const eased = 1 - Math.pow(1 - stream.progress, 2);
        const startX = stream.side === "left" ? width * 0.04 : width * 0.96;
        const startY = centerY + (stream.lane - 0.5) * fieldRadius * 0.46;
        const pullX = centerX + Math.sin(stream.lane * Math.PI * 2 + time * 0.02) * fieldRadius * 0.22;
        const pullY = centerY + Math.cos(stream.lane * Math.PI * 2 + time * 0.018) * fieldRadius * 0.22;
        const x = startX + (pullX - startX) * eased;
        const y = startY + (pullY - startY) * eased + Math.sin(stream.progress * Math.PI * 2) * 8;
        const alpha = clamp(1 - stream.progress, 0, 1) * 0.76;
        context.beginPath();
        context.arc(x, y, stream.size, 0, Math.PI * 2);
        context.fillStyle = stream.side === "left"
          ? `rgba(255, 113, 220, ${alpha})`
          : `rgba(88, 199, 255, ${alpha})`;
        context.shadowBlur = 14;
        context.shadowColor = stream.side === "left" ? "rgba(255, 113, 220, 0.55)" : "rgba(88, 199, 255, 0.55)";
        context.fill();
      }
      streamsRef.current = streamsRef.current.filter((stream) => stream.progress < 1);
      context.shadowBlur = 0;

      const projected = particlesRef.current.map((particle) => {
        const drift = time * particle.speed + particle.phase;
        const theta = particle.theta + drift + Math.sin(time * 0.006 + particle.wobble) * 0.18;
        const phi = particle.phi + Math.sin(drift * 0.9 + particle.wobble) * 0.16;
        const r = fieldRadius * particle.shell;
        const x3 = Math.sin(phi) * Math.cos(theta) * r;
        const y3 = Math.sin(phi) * Math.sin(theta) * r;
        const z3 = Math.cos(phi) * r;
        const depth = 0.62 + (z3 / fieldRadius + 1) * 0.28;
        return {
          particle,
          x: centerX + x3 * 1.05,
          y: centerY + y3 * 0.96,
          z: z3,
          depth,
          drift,
        };
      }).sort((a, b) => a.z - b.z);

      for (const item of projected) {
        const { particle, x, y, depth, drift } = item;
        const alpha = (0.22 + Math.sin(drift * 2) * 0.12 + metrics.totalInfluence * 0.016) * depth;
        context.beginPath();
        context.arc(x, y, particle.size * depth, 0, Math.PI * 2);
        context.fillStyle = colorForParticle(particle, clamp(alpha, 0.22, 0.82));
        context.fill();
      }

      const coreGlow = context.createRadialGradient(centerX, centerY, 0, centerX, centerY, fieldRadius * 0.25);
      coreGlow.addColorStop(0, "rgba(244, 250, 255, 0.72)");
      coreGlow.addColorStop(0.34, "rgba(255, 113, 220, 0.32)");
      coreGlow.addColorStop(0.62, "rgba(88, 199, 255, 0.24)");
      coreGlow.addColorStop(1, "rgba(244, 250, 255, 0)");
      context.beginPath();
      context.arc(centerX, centerY, fieldRadius * 0.4, 0, Math.PI * 2);
      context.fillStyle = coreGlow;
      context.fill();

      context.beginPath();
      context.arc(centerX, centerY, fieldRadius * 1.16, 0, Math.PI * 2);
      context.strokeStyle = `rgba(244, 250, 255, ${0.035 + metrics.totalInfluence * 0.004})`;
      context.lineWidth = 1;
      context.stroke();

      frameRef.current = requestAnimationFrame(draw);
    };

    draw();

    return () => {
      observer.disconnect();
      if (frameRef.current) cancelAnimationFrame(frameRef.current);
    };
  }, [metrics]);

  return (
    <div className="workspace-visual" aria-label="Shared workspace activity visualization">
      <span className="workspace-port workspace-port-left" aria-hidden="true" />
      <canvas ref={canvasRef} className="workspace-visual-canvas" />
      <span className="workspace-port workspace-port-right" aria-hidden="true" />
      <div className="workspace-visual-label">
        <span>Shared Workspace</span>
        <strong>{metrics.totalInfluence > 0 ? "Active influence" : "Neutral idle"}</strong>
      </div>
    </div>
  );
});

export default SharedWorkspaceVisual;
