import { useState, useEffect, useCallback } from "react";

export function MemoryPanel({ apiBase, currentAgentId }) {
  const [query, setQuery] = useState("");
  const [memories, setMemories] = useState([]);
  const [loading, setLoading] = useState(false);
  const [expandedId, setExpandedId] = useState(null);
  const [writeContent, setWriteContent] = useState("");
  const [writeScope, setWriteScope] = useState("agent");
  const [showWriteForm, setShowWriteForm] = useState(false);
  const [suggestions, setSuggestions] = useState([]);
  const [suggestionsLoading, setSuggestionsLoading] = useState(false);
  const [activeTab, setActiveTab] = useState("memory");

  const searchMemories = useCallback(async () => {
    if (!query.trim()) {
      setMemories([]);
      return;
    }
    setLoading(true);
    try {
      const params = new URLSearchParams({ query, limit: "20" });
      if (currentAgentId) params.set("agent", currentAgentId);
      const res = await fetch(`${apiBase}/api/memory?${params}`);
      if (res.ok) {
        const data = await res.json();
        setMemories(data.memories || []);
      }
    } catch (err) {
      console.error("Memory search failed:", err);
    } finally {
      setLoading(false);
    }
  }, [apiBase, query, currentAgentId]);

  useEffect(() => {
    const timer = setTimeout(searchMemories, 300);
    return () => clearTimeout(timer);
  }, [searchMemories]);

  const loadSuggestions = useCallback(async () => {
    setSuggestionsLoading(true);
    try {
      const res = await fetch(`${apiBase}/api/suggestions?status=pending`);
      if (res.ok) {
        const data = await res.json();
        setSuggestions(data.suggestions || []);
      }
    } catch (err) {
      console.error("Suggestions load failed:", err);
    } finally {
      setSuggestionsLoading(false);
    }
  }, [apiBase]);

  useEffect(() => {
    if (activeTab === "suggestions") {
      loadSuggestions();
    }
  }, [activeTab, loadSuggestions]);

  const writeMemory = async (e) => {
    e.preventDefault();
    if (!writeContent.trim()) return;
    try {
      const res = await fetch(`${apiBase}/api/memory`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          content: writeContent,
          scope: writeScope,
          importance: 0.5,
        }),
      });
      if (res.ok) {
        const data = await res.json();
        setMemories((prev) => [data.memory, ...prev]);
        setWriteContent("");
        setShowWriteForm(false);
      }
    } catch (err) {
      console.error("Memory write failed:", err);
    }
  };

  const deleteMemory = async (id) => {
    try {
      const res = await fetch(`${apiBase}/api/memory/${id}`, { method: "DELETE" });
      if (res.ok) {
        setMemories((prev) => prev.filter((m) => m.id !== id));
        if (expandedId === id) setExpandedId(null);
      }
    } catch (err) {
      console.error("Memory delete failed:", err);
    }
  };

  const handleSuggestionAction = async (id, action) => {
    try {
      const res = await fetch(`${apiBase}/api/suggestions/${id}/${action}`, { method: "POST" });
      if (res.ok) {
        setSuggestions((prev) => prev.filter((s) => s.id !== id));
      }
    } catch (err) {
      console.error(`Suggestion ${action} failed:`, err);
    }
  };

  const formatDate = (dateStr) => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  const getTypeColor = (type) => {
    switch (type) {
      case "task": return "var(--accent-primary)";
      case "fact": return "var(--accent-secondary)";
      case "preference": return "var(--accent-purple)";
      default: return "var(--text-secondary)";
    }
  };

  const getSuggestionTypeColor = (type) => {
    switch (type) {
      case "follow_up": return "var(--accent-warning)";
      case "improvement": return "var(--accent-secondary)";
      case "routine": return "var(--accent-primary)";
      case "contextual": return "var(--accent-purple)";
      default: return "var(--text-secondary)";
    }
  };

  return (
    <div className="activity-section">
      <div className="activity-title">
        <span
          style={{
            padding: "2px 6px",
            borderRadius: "3px",
            fontSize: "9px",
            cursor: "pointer",
            background: activeTab === "memory" ? "var(--accent-primary)" : "transparent",
            color: activeTab === "memory" ? "var(--bg-primary)" : "var(--text-secondary)",
          }}
          onClick={() => setActiveTab("memory")}
        >
          Memory
        </span>
        <span
          style={{
            padding: "2px 6px",
            borderRadius: "3px",
            fontSize: "9px",
            cursor: "pointer",
            marginLeft: "4px",
            background: activeTab === "suggestions" ? "var(--accent-warning)" : "transparent",
            color: activeTab === "suggestions" ? "var(--bg-primary)" : "var(--text-secondary)",
          }}
          onClick={() => setActiveTab("suggestions")}
        >
          Suggestions {suggestions.length > 0 && `(${suggestions.length})`}
        </span>
        {activeTab === "memory" && (
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => setShowWriteForm(!showWriteForm)}
            style={{ marginLeft: "auto", fontSize: "10px" }}
          >
            {showWriteForm ? "Cancel" : "+ Write"}
          </button>
        )}
      </div>

      {activeTab === "memory" && (
        <>
          {showWriteForm && (
            <form onSubmit={writeMemory} style={{ marginBottom: "12px", display: "flex", flexDirection: "column", gap: "6px" }}>
              <textarea
                value={writeContent}
                onChange={(e) => setWriteContent(e.target.value)}
                placeholder="Remember that..."
                rows={3}
                style={{
                  width: "100%",
                  padding: "6px 8px",
                  background: "var(--surface-2)",
                  border: "1px solid var(--border-default)",
                  borderRadius: "var(--radius-sm)",
                  color: "var(--text-primary)",
                  fontSize: "11px",
                  resize: "none",
                  fontFamily: "var(--font-ui)",
                }}
              />
              <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
                <select
                  value={writeScope}
                  onChange={(e) => setWriteScope(e.target.value)}
                  style={{
                    padding: "4px 6px",
                    background: "var(--surface-2)",
                    border: "1px solid var(--border-default)",
                    borderRadius: "var(--radius-sm)",
                    color: "var(--text-primary)",
                    fontSize: "10px",
                  }}
                >
                  <option value="agent">Agent</option>
                  <option value="session">Session</option>
                  <option value="user">User</option>
                </select>
                <button type="submit" className="btn btn-confirm btn-sm" style={{ flex: 1 }}>
                  Save
                </button>
              </div>
            </form>
          )}

          <div style={{ position: "relative", marginBottom: "8px" }}>
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search memories..."
              style={{
                width: "100%",
                padding: "6px 8px 6px 24px",
                background: "var(--surface-2)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--radius-sm)",
                color: "var(--text-primary)",
                fontSize: "11px",
              }}
            />
            {loading && (
              <span style={{ position: "absolute", left: "6px", top: "50%", transform: "translateY(-50%)", fontSize: "10px" }}>
                ...
              </span>
            )}
          </div>

          {memories.length === 0 && query && !loading && (
            <div style={{ fontSize: "10px", color: "var(--text-secondary)", textAlign: "center", padding: "12px" }}>
              No memories found
            </div>
          )}

          <div style={{ maxHeight: "300px", overflowY: "auto" }}>
            {memories.map((mem) => (
              <div
                key={mem.id}
                style={{
                  padding: "6px 8px",
                  marginBottom: "4px",
                  background: "var(--surface-2)",
                  borderRadius: "var(--radius-sm)",
                  border: "1px solid var(--border-default)",
                  fontSize: "11px",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "flex-start",
                    cursor: "pointer",
                  }}
                  onClick={() => setExpandedId(expandedId === mem.id ? null : mem.id)}
                >
                  <div style={{ flex: 1, overflow: "hidden" }}>
                    <span
                      style={{
                        display: "inline-block",
                        padding: "1px 4px",
                        borderRadius: "3px",
                        fontSize: "8px",
                        background: `${getTypeColor(mem.memory_type)}22`,
                        color: getTypeColor(mem.memory_type),
                        marginRight: "4px",
                        textTransform: "uppercase",
                      }}
                    >
                      {mem.memory_type}
                    </span>
                    <span style={{ color: "var(--text-secondary)", fontSize: "9px" }}>
                      {formatDate(mem.created_at)}
                    </span>
                    <div
                      style={{
                        color: "var(--text-primary)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: expandedId === mem.id ? "normal" : "nowrap",
                        maxHeight: expandedId === mem.id ? "none" : "20px",
                      }}
                    >
                      {mem.content}
                    </div>
                  </div>
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={(e) => {
                      e.stopPropagation();
                      deleteMemory(mem.id);
                    }}
                    style={{ padding: "2px 4px", fontSize: "9px", color: "var(--accent-danger)" }}
                  >
                    x
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {activeTab === "suggestions" && (
        <>
          {suggestionsLoading ? (
            <div style={{ textAlign: "center", padding: "20px", fontSize: "11px", color: "var(--text-secondary)" }}>
              Loading suggestions...
            </div>
          ) : suggestions.length === 0 ? (
            <div style={{ textAlign: "center", padding: "20px", fontSize: "11px", color: "var(--text-secondary)" }}>
              No pending suggestions
            </div>
          ) : (
            <div style={{ maxHeight: "400px", overflowY: "auto" }}>
              {suggestions.map((sug) => (
                <div
                  key={sug.id}
                  style={{
                    padding: "8px",
                    marginBottom: "6px",
                    background: "var(--surface-2)",
                    borderRadius: "var(--radius-sm)",
                    border: "1px solid var(--border-default)",
                    fontSize: "11px",
                  }}
                >
                  <div style={{ marginBottom: "6px" }}>
                    <span
                      style={{
                        display: "inline-block",
                        padding: "1px 4px",
                        borderRadius: "3px",
                        fontSize: "8px",
                        background: `${getSuggestionTypeColor(sug.suggestion_type)}22`,
                        color: getSuggestionTypeColor(sug.suggestion_type),
                        marginRight: "4px",
                        textTransform: "uppercase",
                      }}
                    >
                      {sug.suggestion_type.replace("_", " ")}
                    </span>
                    <span style={{ color: "var(--text-secondary)", fontSize: "9px" }}>
                      {formatDate(sug.created_at)}
                    </span>
                  </div>
                  <div style={{ color: "var(--text-primary)", marginBottom: "8px", lineHeight: "1.4" }}>
                    {sug.content}
                  </div>
                  {sug.trigger_context && (
                    <div
                      style={{
                        fontSize: "9px",
                        color: "var(--text-secondary)",
                        marginBottom: "8px",
                        padding: "4px",
                        background: "var(--surface-1)",
                        borderRadius: "3px",
                      }}
                    >
                      {sug.trigger_context}
                    </div>
                  )}
                  <div style={{ display: "flex", gap: "6px" }}>
                    <button
                      className="btn btn-confirm btn-sm"
                      style={{ flex: 1, fontSize: "10px" }}
                      onClick={() => handleSuggestionAction(sug.id, "accept")}
                    >
                      Accept
                    </button>
                    <button
                      className="btn btn-ghost btn-sm"
                      style={{ flex: 1, fontSize: "10px" }}
                      onClick={() => handleSuggestionAction(sug.id, "dismiss")}
                    >
                      Dismiss
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}