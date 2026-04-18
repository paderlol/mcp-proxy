import { useEffect, useMemo, useState } from "react";
import { MainContent } from "../components/layout/MainContent";
import {
  listInvocationSessions,
  listInvocationToolCalls,
  invocationCountsByTool,
  listServers,
} from "../lib/tauri";
import type {
  InvocationSession,
  McpServerConfig,
  ToolCallRow,
} from "../lib/types";

export function Activity() {
  const [servers, setServers] = useState<McpServerConfig[]>([]);
  const [sessions, setSessions] = useState<InvocationSession[]>([]);
  const [selectedServer, setSelectedServer] = useState<string | "all">("all");
  const [openSessionId, setOpenSessionId] = useState<number | null>(null);
  const [toolCalls, setToolCalls] = useState<ToolCallRow[]>([]);
  const [toolCounts, setToolCounts] = useState<Array<[string, number]>>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const [srv, sess] = await Promise.all([
        listServers(),
        listInvocationSessions(
          selectedServer === "all" ? undefined : selectedServer,
          100,
        ),
      ]);
      setServers(srv);
      setSessions(sess);
      if (selectedServer !== "all") {
        setToolCounts(await invocationCountsByTool(selectedServer, 7));
      } else {
        setToolCounts([]);
      }
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedServer]);

  useEffect(() => {
    if (openSessionId == null) {
      setToolCalls([]);
      return;
    }
    listInvocationToolCalls(openSessionId, 500)
      .then(setToolCalls)
      .catch((e) => setError(String(e)));
  }, [openSessionId]);

  const serverName = (id: string) =>
    servers.find((s) => s.id === id)?.name ?? id;

  const totalToday = useMemo(() => {
    const since = Date.now() - 24 * 60 * 60 * 1000;
    return sessions.filter((s) => Date.parse(s.started_at) > since).length;
  }, [sessions]);

  return (
    <MainContent
      title="Activity"
      description="Per-session records + JSON-RPC traffic captured from MCP server runs."
      actions={
        <button
          onClick={refresh}
          className="px-3 py-1.5 text-xs rounded-full border border-border-default/50 text-text-secondary hover:text-text-primary hover:border-border-default transition-colors"
        >
          Refresh
        </button>
      }
    >
      {error && (
        <div className="mb-4 p-3 rounded-md bg-red-500/10 text-red-400 text-sm">
          {error}
        </div>
      )}

      {/* Filter */}
      <div className="mb-6 flex items-center gap-3">
        <label className="text-sm text-text-secondary">Server:</label>
        <select
          value={selectedServer}
          onChange={(e) => {
            setSelectedServer(e.target.value);
            setOpenSessionId(null);
          }}
          className="bg-bg-elevated text-text-primary text-sm px-3 py-1.5 rounded-md border border-border-default/40 focus:outline-none focus:border-brand/60"
        >
          <option value="all">All servers</option>
          {servers.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <span className="text-xs text-text-secondary">
          {sessions.length} session{sessions.length === 1 ? "" : "s"}
          {selectedServer === "all" ? " total" : ""} · {totalToday} today
        </span>
      </div>

      {/* Tool histogram (only when a specific server is selected) */}
      {selectedServer !== "all" && toolCounts.length > 0 && (
        <div className="mb-6 p-4 rounded-lg bg-bg-elevated border border-border-default/30">
          <h2 className="text-sm font-bold text-text-primary mb-3">
            Top tools (last 7 days)
          </h2>
          <ul className="flex flex-wrap gap-2">
            {toolCounts.slice(0, 10).map(([name, count]) => (
              <li
                key={name}
                className="px-2.5 py-1 text-xs rounded-full bg-bg-base border border-border-default/40 text-text-secondary"
              >
                <span className="text-text-primary">{name}</span> · {count}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Sessions table */}
      <div className="rounded-lg bg-bg-elevated border border-border-default/30 overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-bg-base/60 text-text-secondary">
            <tr>
              <th className="text-left px-4 py-2 font-medium">Server</th>
              <th className="text-left px-4 py-2 font-medium">Mode</th>
              <th className="text-left px-4 py-2 font-medium">Started</th>
              <th className="text-left px-4 py-2 font-medium">Calls</th>
              <th className="text-left px-4 py-2 font-medium">Status</th>
            </tr>
          </thead>
          <tbody>
            {sessions.length === 0 ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-4 py-6 text-center text-text-secondary text-sm"
                >
                  No sessions recorded yet. Run an MCP server via an AI client
                  to populate this list.
                </td>
              </tr>
            ) : (
              sessions.map((s) => (
                <tr
                  key={s.id}
                  onClick={() =>
                    setOpenSessionId(openSessionId === s.id ? null : s.id)
                  }
                  className={`border-t border-border-default/20 cursor-pointer hover:bg-bg-base/50 ${
                    openSessionId === s.id ? "bg-bg-base/70" : ""
                  }`}
                >
                  <td className="px-4 py-2 text-text-primary">
                    {serverName(s.server_id)}
                  </td>
                  <td className="px-4 py-2 text-text-secondary">
                    {s.run_mode}
                  </td>
                  <td className="px-4 py-2 text-text-secondary">
                    {new Date(s.started_at).toLocaleString()}
                  </td>
                  <td className="px-4 py-2 text-text-primary">
                    {s.tool_call_count}
                  </td>
                  <td className="px-4 py-2">
                    {s.ended_at == null ? (
                      <span className="text-yellow-400">running</span>
                    ) : s.error != null ? (
                      <span className="text-red-400" title={s.error}>
                        error
                      </span>
                    ) : s.exit_code === 0 ? (
                      <span className="text-brand">ok</span>
                    ) : (
                      <span className="text-text-secondary">
                        exit {s.exit_code}
                      </span>
                    )}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>

        {openSessionId != null && (
          <div className="border-t border-border-default/30 p-4 bg-bg-base/60">
            <h3 className="text-sm font-bold text-text-primary mb-3">
              Tool calls
            </h3>
            {toolCalls.length === 0 ? (
              <p className="text-xs text-text-secondary">
                No traffic captured for this session.
              </p>
            ) : (
              <ul className="space-y-2 max-h-96 overflow-y-auto">
                {toolCalls.map((tc) => (
                  <li
                    key={tc.id}
                    className="p-2 rounded bg-bg-elevated border border-border-default/20"
                  >
                    <div className="flex items-center gap-3 text-xs">
                      <span
                        className={`px-1.5 py-0.5 rounded ${
                          tc.direction === "request"
                            ? "bg-blue-500/20 text-blue-300"
                            : tc.direction === "response"
                              ? "bg-brand/20 text-brand"
                              : "bg-text-secondary/10 text-text-secondary"
                        }`}
                      >
                        {tc.direction}
                      </span>
                      <span className="text-text-primary font-mono">
                        {tc.method ?? "(unparsed)"}
                      </span>
                      {tc.tool_name && (
                        <span className="text-brand">{tc.tool_name}</span>
                      )}
                      {tc.duration_ms != null && (
                        <span className="text-text-secondary">
                          {tc.duration_ms}ms
                        </span>
                      )}
                      {tc.is_error && <span className="text-red-400">err</span>}
                      <span className="ml-auto text-text-secondary/60">
                        {new Date(tc.timestamp).toLocaleTimeString()}
                      </span>
                    </div>
                    <details className="mt-1">
                      <summary className="cursor-pointer text-[10px] text-text-secondary/70 hover:text-text-secondary">
                        payload
                      </summary>
                      <pre className="mt-1 text-[11px] text-text-secondary whitespace-pre-wrap break-all max-h-40 overflow-y-auto">
                        {tc.payload}
                      </pre>
                    </details>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </MainContent>
  );
}
