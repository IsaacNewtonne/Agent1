import { useCallback, useEffect, useMemo, useRef, useState } from "react";

const ACTIVE_PROJECT_KEY = "agent1_active_project";

function displayModelContent(content) {
  if (typeof content !== "string") return "";
  const trimmed = content.trim();
  if (!trimmed.startsWith("{")) return content;
  try {
    const parsed = JSON.parse(trimmed);
    if (typeof parsed.final === "string") return parsed.final;
  } catch {
    // Keep the original content when it is not model-action JSON.
  }
  return content;
}

/**
 * Custom hook for collaboration workspace state management.
 * Fetches projects, agents, external agents, and blackboard state.
 * Manages WebSocket connection for live updates.
 */
export default function useCollaboration(apiBase) {
  const [projects, setProjects] = useState([]);
  const [activeProject, setActiveProjectState] = useState(null);
  const [agents, setAgents] = useState([]);
  const [mcpServers, setMcpServers] = useState([]);
  const [modelProviders, setModelProviders] = useState([]);
  const [externals, setExternals] = useState([]);
  const [inviteTokens, setInviteTokens] = useState([]);
  const [collabTasks, setCollabTasks] = useState([]);
  const [blackboardEntries, setBlackboardEntries] = useState([]);
  const [activeSessions, setActiveSessions] = useState([]);
  const [recentEvents, setRecentEvents] = useState([]);
  const [trace, setTrace] = useState({ messages: [], events: [], approvals: [] });
  const [crocStatus, setCrocStatus] = useState({ available: false, version: "", error: "" });
  const [stripeStatus, setStripeStatus] = useState({ configured: false, mode: "dry_run", capabilities: [] });
  const [wsState, setWsState] = useState("disconnected");
  const [loading, setLoading] = useState(true);
  const wsRef = useRef(null);
  const activeProjectRef = useRef(null);
  const modelRefreshRef = useRef(false);
  const refreshInFlightRef = useRef(false);
  const pendingRefreshRef = useRef(false);
  const lastRefreshAtRef = useRef(0);

  const fetchJson = useCallback(async (path, options = {}) => {
    const response = await fetch(`${apiBase}${path}`, options);
    if (!response.ok) {
      let message = `${path} returned ${response.status}`;
      try {
        const body = await response.json();
        message = body?.error?.message || body?.message || message;
      } catch {
        try {
          const text = await response.text();
          if (text.trim()) message = text.trim();
        } catch {
          // Keep the status fallback.
        }
      }
      throw new Error(message);
    }
    return response.json();
  }, [apiBase]);

  useEffect(() => {
    activeProjectRef.current = activeProject;
  }, [activeProject]);

  // ─── Refresh loop ───

  const refreshAll = useCallback(async () => {
    if (refreshInFlightRef.current) {
      pendingRefreshRef.current = true;
      return;
    }
    refreshInFlightRef.current = true;
    try {
      const [agentsRes, sessionsRes, eventsRes, mcpRes, approvalsRes] = await Promise.all([
        fetchJson("/api/agents"),
        fetchJson("/api/sessions"),
        fetchJson("/api/events"),
        fetchJson("/api/mcp/servers"),
        fetchJson("/api/approvals"),
      ]);

      setAgents(agentsRes.agents || []);
      setMcpServers(mcpRes.servers || []);

      const running = (sessionsRes.sessions || []).filter((s) => s.status === "running");
      setActiveSessions(running);
      setRecentEvents((eventsRes.events || []).slice(-30).reverse());

      // Fetch latest session trace
      const latestSession = (sessionsRes.sessions || [])[0];
      if (latestSession) {
        try {
          const traceData = await fetchJson(`/api/sessions/${encodeURIComponent(latestSession.id)}/trace`);
          setTrace({
            session: traceData.session || latestSession,
            messages: traceData.messages || [],
            tool_calls: traceData.tool_calls || [],
            events: traceData.events || [],
            approvals: traceData.approvals || approvalsRes.approvals || [],
          });
        } catch {
          setTrace((prev) => ({ ...prev, approvals: approvalsRes.approvals || [] }));
        }
      } else {
        setTrace({ session: null, messages: [], tool_calls: [], events: [], approvals: approvalsRes.approvals || [] });
      }

      // Try fetching projects (may not exist on older backends)
      try {
        const projRes = await fetchJson("/api/projects");
        const nextProjects = projRes.projects || projRes || [];
        setProjects(nextProjects);
        if (!nextProjects.length) {
          localStorage.removeItem(ACTIVE_PROJECT_KEY);
          setActiveProjectState(null);
          setExternals([]);
          setInviteTokens([]);
          setCollabTasks([]);
          setBlackboardEntries([]);
        } else {
          const savedId = localStorage.getItem(ACTIVE_PROJECT_KEY);
          const selected =
            nextProjects.find((project) => project.id === activeProjectRef.current?.id) ||
            nextProjects.find((project) => project.id === savedId) ||
            nextProjects[0];
          if (selected?.id) localStorage.setItem(ACTIVE_PROJECT_KEY, selected.id);
          setActiveProjectState(selected);

          const projectId = encodeURIComponent(selected.id);
          const [externalsRes, invitesRes, tasksRes, blackboardRes] = await Promise.all([
            fetchJson(`/api/projects/${projectId}/externals`).catch(() => ({ externals: [] })),
            fetchJson(`/api/projects/${projectId}/invites`).catch(() => ({ invites: [] })),
            fetchJson(`/api/projects/${projectId}/tasks`).catch(() => ({ tasks: [] })),
            fetchJson(`/api/projects/${projectId}/blackboard`).catch(() => ({ entries: [] })),
          ]);
          setExternals(externalsRes.externals || []);
          setInviteTokens(invitesRes.invites || []);
          setCollabTasks(tasksRes.tasks || []);
          setBlackboardEntries(blackboardRes.entries || []);
        }
      } catch {
        // Project API not available yet — use a default
        setProjects([]);
        setActiveProjectState(null);
        setExternals([]);
        setInviteTokens([]);
        setCollabTasks([]);
        setBlackboardEntries([]);
      }

      if (!modelRefreshRef.current) {
        modelRefreshRef.current = true;
        fetchJson("/api/models")
          .then((modelsRes) => setModelProviders(modelsRes.providers || []))
          .catch(() => setModelProviders([]))
          .finally(() => {
            modelRefreshRef.current = false;
          });
      }

      fetchJson("/api/croc/status")
        .then((status) => setCrocStatus(status || { available: false }))
        .catch((error) => setCrocStatus({ available: false, error: error.message }));

      fetchJson("/api/stripe/status")
        .then((status) => setStripeStatus(status || { configured: false, mode: "dry_run", capabilities: [] }))
        .catch((error) => setStripeStatus({ configured: false, mode: "unavailable", error: error.message, capabilities: [] }));

      setLoading(false);
    } catch (error) {
      console.error("Refresh failed:", error);
      setLoading(false);
    } finally {
      lastRefreshAtRef.current = Date.now();
      refreshInFlightRef.current = false;
      if (pendingRefreshRef.current) {
        pendingRefreshRef.current = false;
        setTimeout(() => {
          refreshAll();
        }, 120);
      }
    }
  }, [fetchJson]);

  useEffect(() => {
    refreshAll();
    const timer = setInterval(() => {
      if (wsState !== "connected") {
        refreshAll();
      }
    }, 15000);
    return () => clearInterval(timer);
  }, [refreshAll, wsState]);

  // ─── WebSocket ───

  useEffect(() => {
    const wsBase = apiBase.replace(/^http/i, "ws");
    const ws = new WebSocket(`${wsBase}/ws/events`);
    let disposed = false;
    wsRef.current = ws;

    const handleOpen = () => {
      if (disposed) {
        ws.close();
        return;
      }
      setWsState("connected");
    };
    const handleClose = () => {
      if (!disposed) setWsState("disconnected");
    };
    const handleError = () => {
      if (!disposed) setWsState("error");
    };
    const handleMessage = (event) => {
      if (disposed) return;
      try {
        const data = JSON.parse(event.data);
        const runtimeEvent = data.type === "event" && data.event ? data.event : data;
        if (runtimeEvent.event_type) {
          setRecentEvents((prev) => {
            if (prev.some((item) => item.id === runtimeEvent.id)) return prev;
            return [runtimeEvent, ...prev].slice(0, 30);
          });
        }
        // Auto-refresh on important events with throttling to prevent refresh storms.
        if (["SessionStarted", "FinalAnswer", "ToolCallCompleted", "Error"].includes(runtimeEvent.event_type)) {
          const now = Date.now();
          if (now - lastRefreshAtRef.current > 700) {
            refreshAll();
          } else {
            pendingRefreshRef.current = true;
          }
        }
      } catch { /* ignore malformed */ }
    };

    ws.addEventListener("open", handleOpen);
    ws.addEventListener("close", handleClose);
    ws.addEventListener("error", handleError);
    ws.addEventListener("message", handleMessage);

    return () => {
      disposed = true;
      if (wsRef.current === ws) wsRef.current = null;
      ws.removeEventListener("open", handleOpen);
      ws.removeEventListener("close", handleClose);
      ws.removeEventListener("error", handleError);
      ws.removeEventListener("message", handleMessage);
      if (ws.readyState === WebSocket.CONNECTING) {
        ws.onopen = () => ws.close();
      } else if (ws.readyState === WebSocket.OPEN) {
        ws.close();
      }
    };
  }, [apiBase, refreshAll]);

  // ─── Derived state ───

  const runningAgentIds = useMemo(
    () => new Set(activeSessions.map((s) => s.root_agent_id)),
    [activeSessions]
  );

  const agent1Agent = useMemo(() => {
    return (
      agents.find((a) => a.id?.toLowerCase?.() === "agent1") ||
      agents.find((a) => a.name?.toLowerCase?.() === "agent1") ||
      agents.find((a) => a.role?.toLowerCase?.()?.includes("orchestrator")) ||
      null
    );
  }, [agents]);

  const localAgents = useMemo(
    () => agents.filter((a) => a.id !== agent1Agent?.id),
    [agents, agent1Agent]
  );

  const pendingApprovals = useMemo(
    () => (trace.approvals || []).filter((a) => !a.decision),
    [trace.approvals]
  );

  const streamingOutput = useMemo(() => {
    const msgs = (trace.messages || []).filter((m) => m.role === "assistant");
    return msgs.length > 0 ? displayModelContent(msgs[msgs.length - 1].content) : "";
  }, [trace.messages]);

  // ─── Actions ───

  const createProject = useCallback(async (name, mode = "automatic") => {
    try {
      const result = await fetchJson("/api/projects", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name, collaboration_mode: mode }),
      });
      const project = result.project || result;
      if (project?.id) {
        setActiveProjectState(project);
        localStorage.setItem(ACTIVE_PROJECT_KEY, project.id);
      }
      await refreshAll();
      return project;
    } catch (error) {
      console.error("Failed to create project:", error);
      throw error;
    }
  }, [fetchJson, refreshAll]);

  const setActiveProject = useCallback((project) => {
    setActiveProjectState(project);
    if (project?.id) {
      localStorage.setItem(ACTIVE_PROJECT_KEY, project.id);
    }
  }, []);

  const runTask = useCallback(async (input, agentId, workspace = ".", projectId = activeProject?.id) => {
    try {
      const result = await fetchJson("/api/sessions/run", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          agent_id: agentId || agent1Agent?.id || "agent1",
          project_id: projectId || undefined,
          input,
          workspace,
        }),
      });
      await refreshAll();
      return {
        ...result,
        final_answer: result.final_answer || result.final || "",
      };
    } catch (error) {
      console.error("Run failed:", error);
      throw error;
    }
  }, [fetchJson, refreshAll, agent1Agent, activeProject]);

  const approveAction = useCallback(async (approvalId, approved) => {
    try {
      await fetchJson(`/api/tool-approvals/${encodeURIComponent(approvalId)}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ decision: approved ? "approved" : "denied" }),
      });
      await refreshAll();
    } catch (error) {
      console.error("Approval failed:", error);
    }
  }, [fetchJson, refreshAll]);

  const saveAgent = useCallback(async (agentPayload) => {
    try {
      await fetchJson("/api/agents", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(agentPayload),
      });
      await refreshAll();
    } catch (error) {
      console.error("Save agent failed:", error);
      throw error;
    }
  }, [fetchJson, refreshAll]);

  const deleteAgent = useCallback(async (agentId) => {
    try {
      await fetchJson(`/api/agents/${encodeURIComponent(agentId)}`, {
        method: "DELETE",
      });
      await refreshAll();
    } catch (error) {
      console.error("Delete agent failed:", error);
      throw error;
    }
  }, [fetchJson, refreshAll]);

  const saveMcpServer = useCallback(async (serverPayload) => {
    try {
      await fetchJson("/api/mcp/servers", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(serverPayload),
      });
      await refreshAll();
    } catch (error) {
      console.error("Save MCP server failed:", error);
      throw error;
    }
  }, [fetchJson, refreshAll]);

  const createInvite = useCallback(async (permissions, createdBy = "user", expiresAt = null) => {
    if (!activeProject?.id) throw new Error("Create a project before inviting external agents.");
    const result = await fetchJson(`/api/projects/${encodeURIComponent(activeProject.id)}/invite`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ created_by: createdBy, permissions, expires_at: expiresAt }),
    });
    await refreshAll();
    return result;
  }, [fetchJson, refreshAll, activeProject]);

  const revokeInvite = useCallback(async (token) => {
    if (!activeProject?.id) throw new Error("No active project.");
    await fetchJson(`/api/projects/${encodeURIComponent(activeProject.id)}/invites/${encodeURIComponent(token)}`, {
      method: "DELETE",
    });
    await refreshAll();
  }, [fetchJson, refreshAll, activeProject]);

  const updateMcpServer = useCallback(async (serverId, patch) => {
    await fetchJson(`/api/mcp/servers/${encodeURIComponent(serverId)}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
    });
    await refreshAll();
  }, [fetchJson, refreshAll]);

  const deleteMcpServer = useCallback(async (serverId) => {
    await fetchJson(`/api/mcp/servers/${encodeURIComponent(serverId)}`, {
      method: "DELETE",
    });
    await refreshAll();
  }, [fetchJson, refreshAll]);

  const checkMcpHealth = useCallback(async (serverId) => {
    return fetchJson(`/api/mcp/servers/${encodeURIComponent(serverId)}/health`);
  }, [fetchJson]);

  const listMcpTools = useCallback(async (serverId) => {
    return fetchJson(`/api/mcp/servers/${encodeURIComponent(serverId)}/tools`);
  }, [fetchJson]);

  const secureSendSession = useCallback(async (sessionId, options = {}) => {
    const result = await fetchJson(`/api/sessions/${encodeURIComponent(sessionId)}/secure-send`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(options),
    });
    await refreshAll();
    return result;
  }, [fetchJson, refreshAll]);

  const createStripeCheckout = useCallback(async (options = {}) => {
    const result = await fetchJson("/api/stripe/checkout-session", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        project_id: activeProject?.id,
        ...options,
      }),
    });
    await refreshAll();
    return result;
  }, [fetchJson, refreshAll, activeProject]);

  return {
    // State
    projects,
    activeProject,
    agents,
    localAgents,
    agent1Agent,
    mcpServers,
    modelProviders,
    externals,
    inviteTokens,
    collabTasks,
    blackboardEntries,
    activeSessions,
    runningAgentIds,
    recentEvents,
    trace,
    crocStatus,
    stripeStatus,
    pendingApprovals,
    streamingOutput,
    wsState,
    loading,

    // Actions
    createProject,
    setActiveProject,
    runTask,
    approveAction,
    saveAgent,
    deleteAgent,
    saveMcpServer,
    createInvite,
    revokeInvite,
    updateMcpServer,
    deleteMcpServer,
    checkMcpHealth,
    listMcpTools,
    secureSendSession,
    createStripeCheckout,
    refreshAll,
    fetchJson,
  };
}
