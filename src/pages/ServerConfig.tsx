import { useState, useEffect } from "react";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { PillButton } from "../components/ui/PillButton";
import { SearchInput } from "../components/ui/SearchInput";
import { Modal } from "../components/ui/Modal";
import { Badge } from "../components/ui/Badge";
import { RegistryBrowser } from "../components/servers/RegistryBrowser";
import {
  Plus,
  Server,
  Monitor,
  Container,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  LibraryBig,
  KeyRound,
  X,
  Pencil,
} from "lucide-react";
import { useServers } from "../hooks/useServers";
import { useSecrets } from "../hooks/useSecrets";
import type { EnvMapping, McpServerConfig } from "../lib/types";
import type { RegistryEntry } from "../data/registry";

type RunModeType = "Local" | "DockerSandbox";

export function ServerConfig() {
  const { servers, fetchServers, addServer, updateServer, deleteServer } =
    useServers();
  const { secrets, fetchSecrets } = useSecrets();

  const [showAdd, setShowAdd] = useState(false);
  const [showRegistry, setShowRegistry] = useState(false);
  const [prefillEntry, setPrefillEntry] = useState<RegistryEntry | null>(null);
  // null = adding a new server; non-null = editing this existing server
  const [editingServer, setEditingServer] = useState<McpServerConfig | null>(
    null,
  );
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [transportType, setTransportType] = useState("stdio");
  const [runMode, setRunMode] = useState<RunModeType>("Local");
  const [dockerImage, setDockerImage] = useState("");
  const [envMappings, setEnvMappings] = useState<EnvMapping[]>([]);
  const [trusted, setTrusted] = useState(false);
  const [saving, setSaving] = useState(false);
  const [query, setQuery] = useState("");
  const [pendingSave, setPendingSave] = useState(false);

  useEffect(() => {
    fetchServers();
    fetchSecrets();
  }, [fetchServers, fetchSecrets]);

  const resetForm = () => {
    setName("");
    setCommand("");
    setArgs("");
    setTransportType("stdio");
    setRunMode("Local");
    setDockerImage("");
    setEnvMappings([]);
    setTrusted(false);
    setPrefillEntry(null);
    setEditingServer(null);
    setShowAdd(false);
    setPendingSave(false);
  };

  const handleEdit = (server: McpServerConfig) => {
    setEditingServer(server);
    setName(server.name);
    setCommand(server.command);
    setArgs(server.args.join(" "));
    setTransportType(server.transport.type.toLowerCase());
    if (server.run_mode.type === "DockerSandbox") {
      setRunMode("DockerSandbox");
      setDockerImage(server.run_mode.image ?? "");
    } else {
      setRunMode("Local");
      setDockerImage("");
    }
    setEnvMappings([...server.env_mappings]);
    setTrusted(server.trusted);
    setPrefillEntry(null);
    setShowAdd(true);
  };

  const handleInstallFromRegistry = (entry: RegistryEntry) => {
    setName(entry.name);
    setCommand(entry.command);
    setArgs(entry.args.join(" "));
    setTransportType("stdio");
    setRunMode("Local");
    setDockerImage("");
    // Pre-fill env var names from the registry entry; user picks secrets next
    setEnvMappings(
      entry.envVars.map((v) => ({
        env_var_name: v.name,
        secret_ref: "",
      })),
    );
    setTrusted(false);
    setPrefillEntry(entry);
    setShowAdd(true);
  };

  const addEnvMapping = () => {
    setEnvMappings((prev) => [
      ...prev,
      { env_var_name: "", secret_ref: "" },
    ]);
  };

  const updateEnvMapping = (
    index: number,
    field: "env_var_name" | "secret_ref",
    value: string,
  ) => {
    setEnvMappings((prev) =>
      prev.map((m, i) => (i === index ? { ...m, [field]: value } : m)),
    );
  };

  const removeEnvMapping = (index: number) => {
    setEnvMappings((prev) => prev.filter((_, i) => i !== index));
  };

  const persistServer = async () => {
    if (!name || !command) return;
    setSaving(true);
    try {
      // Filter out incomplete mappings (empty env var name or secret ref)
      const validMappings = envMappings.filter(
        (m) => m.env_var_name.trim() && m.secret_ref.trim(),
      );
      const argList = args.split(/\s+/).filter((a) => a.length > 0);

      if (editingServer) {
        // Preserve immutable fields (id, created_at, enabled, trusted)
        await updateServer({
          ...editingServer,
          name,
          command,
          args: argList,
          transport:
            transportType === "sse"
              ? { type: "Sse", port: 3000, path: "/sse" }
              : { type: "Stdio" },
          run_mode:
            runMode === "DockerSandbox"
              ? {
                  type: "DockerSandbox",
                  image: dockerImage || null,
                  extra_args:
                    editingServer.run_mode.type === "DockerSandbox"
                      ? editingServer.run_mode.extra_args
                      : [],
                }
              : { type: "Local" },
          env_mappings: validMappings,
          trusted,
          updated_at: new Date().toISOString(),
        });
      } else {
        await addServer({
          name,
          command,
          args: argList,
          transportType,
          runModeType: runMode === "DockerSandbox" ? "docker" : undefined,
          dockerImage: dockerImage || undefined,
          envMappings: validMappings,
          trusted,
        });
      }
      resetForm();
    } finally {
      setSaving(false);
    }
  };

  const handleSave = async () => {
    if (!name || !command) return;
    if (!trusted) {
      setPendingSave(true);
      return;
    }
    await persistServer();
  };

  // Filtered servers based on the search box.
  const filteredServers = query.trim()
    ? servers.filter((s) => {
        const haystack = [
          s.name,
          s.id,
          s.command,
          s.args.join(" "),
          s.run_mode.type === "DockerSandbox" ? s.run_mode.image ?? "" : "",
        ]
          .join(" ")
          .toLowerCase();
        return haystack.includes(query.toLowerCase().trim());
      })
    : servers;

  const inputClass =
    "w-full bg-bg-elevated text-text-primary rounded-[500px] px-4 py-2.5 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] placeholder:text-text-secondary/50";

  const selectClass =
    "bg-bg-elevated text-text-primary rounded-[500px] px-3 py-2.5 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] appearance-none cursor-pointer";

  return (
    <MainContent
      title="Servers"
      description="Configure MCP servers and their environment variables"
      actions={
        <>
          <PillButton variant="outlined" onClick={() => setShowRegistry(true)}>
            <LibraryBig size={14} className="mr-1.5" />
            Browse
          </PillButton>
          <PillButton variant="brand" onClick={() => setShowAdd(true)}>
            <Plus size={14} className="mr-1.5" />
            Add Server
          </PillButton>
        </>
      }
    >
      <div className="mb-6">
        <SearchInput
          placeholder="Search by name, command, image…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {servers.length === 0 ? (
        <Card>
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <Server size={40} className="text-text-secondary/40 mb-3" />
            <p className="text-sm text-text-secondary mb-1">
              No MCP servers configured yet
            </p>
            <p className="text-xs text-text-secondary/60">
              Click "Browse" to pick from a registry, or "Add Server" to configure manually
            </p>
          </div>
        </Card>
      ) : filteredServers.length === 0 ? (
        <Card>
          <div className="flex flex-col items-center justify-center py-10 text-center">
            <p className="text-sm text-text-secondary">
              No servers match "{query}"
            </p>
          </div>
        </Card>
      ) : (
        <div className="flex flex-col gap-3">
          {filteredServers.map((s) => {
            return (
              <Card key={s.id}>
                <div className="flex items-center justify-between">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1 flex-wrap">
                      <p className="text-sm font-bold text-text-primary">
                        {s.name}
                      </p>
                      <Badge>
                        {s.run_mode.type === "Local" ? "Local" : "Docker"}
                      </Badge>
                      <Badge variant={s.trusted ? "success" : "warning"}>
                        {s.trusted ? "Trusted" : "Untrusted"}
                      </Badge>
                      {s.env_mappings.length > 0 && (
                        <Badge>
                          <KeyRound size={10} className="mr-1" />
                          {s.env_mappings.length} env
                        </Badge>
                      )}
                    </div>
                    <p className="text-xs text-text-secondary font-mono truncate">
                      {s.command} {s.args.join(" ")}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 ml-4">
                    <PillButton
                      variant="outlined"
                      onClick={() => handleEdit(s)}
                      aria-label={`Edit ${s.name}`}
                    >
                      <Pencil size={12} />
                    </PillButton>
                    <PillButton
                      variant="outlined"
                      onClick={() => deleteServer(s.id)}
                      className="!text-negative hover:!border-negative"
                      aria-label={`Delete ${s.name}`}
                    >
                      <Trash2 size={12} />
                    </PillButton>
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      )}

      <Modal
        open={showAdd}
        onClose={resetForm}
        title={
          editingServer
            ? `Edit Server: ${editingServer.name}`
            : prefillEntry
              ? `Install ${prefillEntry.name}`
              : "Add MCP Server"
        }
        size="lg"
      >
        <div className="flex flex-col gap-4">
          {prefillEntry && prefillEntry.envVars.length > 0 && (
            <div className="bg-bg-elevated rounded-lg p-3 border border-warning/30">
              <div className="flex items-center gap-2 mb-2">
                <KeyRound size={14} className="text-warning" />
                <p className="text-sm font-bold text-text-primary">
                  Required Secrets
                </p>
              </div>
              <p className="text-xs text-text-secondary/80 mb-2">
                This server needs these env vars. The names are pre-filled below —
                pick which secret provides each value.
              </p>
              <ul className="flex flex-col gap-1">
                {prefillEntry.envVars.map((v) => (
                  <li key={v.name} className="text-xs">
                    <code className="text-text-bright font-bold">{v.name}</code>
                    {v.required && <span className="text-warning ml-1.5">*</span>}
                    <span className="text-text-secondary ml-2">
                      {v.description}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Server Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., GitHub MCP"
              className={inputClass}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Command
            </label>
            <input
              type="text"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="e.g., npx"
              className={inputClass}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Arguments
            </label>
            <input
              type="text"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder="e.g., -y @modelcontextprotocol/server-github"
              className={inputClass}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Transport
            </label>
            <div className="flex gap-2">
              <PillButton
                variant={transportType === "stdio" ? "dark" : "outlined"}
                onClick={() => setTransportType("stdio")}
                className="flex-1"
              >
                Stdio
              </PillButton>
              <PillButton
                variant={transportType === "sse" ? "dark" : "outlined"}
                onClick={() => setTransportType("sse")}
                className="flex-1"
              >
                SSE
              </PillButton>
            </div>
          </div>

          <div className="border-t border-border-default/30 pt-4">
            <label className="text-sm text-text-secondary font-bold mb-2 block">
              Trust Level
            </label>
            <div className="flex gap-2 mb-2">
              <PillButton
                variant={!trusted ? "dark" : "outlined"}
                onClick={() => setTrusted(false)}
                className="flex-1"
              >
                <ShieldAlert size={14} className="mr-1.5" />
                Untrusted
              </PillButton>
              <PillButton
                variant={trusted ? "brand" : "outlined"}
                onClick={() => setTrusted(true)}
                className="flex-1"
              >
                <ShieldCheck size={14} className="mr-1.5" />
                Trusted
              </PillButton>
            </div>
            <p className="text-xs text-text-secondary/60 px-1">
              {trusted
                ? "Marked as reviewed. This suppresses the untrusted-server warning."
                : "Untrusted servers should be reviewed before you expose them to any AI client."}
            </p>
          </div>

          <div className="border-t border-border-default/30 pt-4">
            <label className="text-sm text-text-secondary font-bold mb-2 block">
              Run Mode
            </label>
            <div className="flex gap-2 mb-2">
              <PillButton
                variant={runMode === "Local" ? "dark" : "outlined"}
                onClick={() => setRunMode("Local")}
                className="flex-1"
              >
                <Monitor size={14} className="mr-1.5" />
                Local
              </PillButton>
              <PillButton
                variant={runMode === "DockerSandbox" ? "dark" : "outlined"}
                onClick={() => setRunMode("DockerSandbox")}
                className="flex-1"
              >
                <Container size={14} className="mr-1.5" />
                Docker Sandbox
              </PillButton>
            </div>
            <p className="text-xs text-text-secondary/60 px-1">
              {runMode === "Local" ? (
                <>
                  <ShieldAlert
                    size={12}
                    className="inline mr-1 text-warning"
                  />
                  Direct process — fast but no isolation.
                </>
              ) : (
                <>
                  <ShieldCheck size={12} className="inline mr-1 text-brand" />
                  Docker container — filesystem and network isolated.
                </>
              )}
            </p>
            {runMode === "DockerSandbox" && (
              <div className="mt-3 flex flex-col gap-1.5">
                <label className="text-sm text-text-secondary font-bold">
                  Docker Image <span className="text-negative">*</span>
                </label>
                <input
                  type="text"
                  value={dockerImage}
                  onChange={(e) => setDockerImage(e.target.value)}
                  placeholder="e.g., node:20-alpine or python:3.12-alpine"
                  className={inputClass}
                />
                {!dockerImage.trim() ? (
                  <p className="text-xs text-warning flex items-center gap-1 px-1">
                    <ShieldAlert size={12} className="inline" />
                    A base image is required for sandbox mode. Pick one with
                    the runtime your command needs (e.g.,{" "}
                    <code className="text-text-bright">node:20-alpine</code>{" "}
                    for <code className="text-text-bright">npx</code>).
                  </p>
                ) : (
                  <p className="text-xs text-text-secondary/60 px-1">
                    First build wraps this image with our sandbox agent (~2
                    min). Subsequent runs are cached.
                  </p>
                )}
              </div>
            )}
          </div>

          {/* Env Mappings */}
          <div className="border-t border-border-default/30 pt-4">
            <div className="flex items-center justify-between mb-2">
              <label className="text-sm text-text-secondary font-bold">
                Environment Variables
              </label>
              <PillButton variant="outlined" onClick={addEnvMapping}>
                <Plus size={12} className="mr-1" />
                Add Mapping
              </PillButton>
            </div>
            <p className="text-xs text-text-secondary/60 mb-3">
              Map env var names to secrets. At runtime,{" "}
              <code className="text-text-bright">mcp-proxy run</code>{" "}
              resolves them and injects as env vars.
            </p>

            {envMappings.length === 0 ? (
              <p className="text-xs text-text-secondary/50 italic py-2 text-center">
                No env mappings. Click "Add Mapping" to link a secret.
              </p>
            ) : (
              <div className="flex flex-col gap-2">
                {envMappings.map((mapping, idx) => (
                  <div key={idx} className="flex items-center gap-2">
                    <input
                      type="text"
                      value={mapping.env_var_name}
                      onChange={(e) =>
                        updateEnvMapping(idx, "env_var_name", e.target.value)
                      }
                      placeholder="ENV_VAR_NAME"
                      className={`${inputClass} flex-1 font-mono`}
                    />
                    <span className="text-text-secondary/60 text-sm">→</span>
                    <select
                      value={mapping.secret_ref}
                      onChange={(e) =>
                        updateEnvMapping(idx, "secret_ref", e.target.value)
                      }
                      className={`${selectClass} flex-1`}
                    >
                      <option value="">Select a secret…</option>
                      {secrets.map((s) => (
                        <option key={s.id} value={s.id}>
                          {s.label} ({s.id})
                        </option>
                      ))}
                    </select>
                    <button
                      type="button"
                      onClick={() => removeEnvMapping(idx)}
                      className="text-text-secondary hover:text-negative p-1.5 rounded-full hover:bg-bg-elevated transition-colors"
                      aria-label="Remove mapping"
                    >
                      <X size={14} />
                    </button>
                  </div>
                ))}
              </div>
            )}

            {envMappings.length > 0 && secrets.length === 0 && (
              <p className="text-xs text-warning/80 mt-2">
                No secrets available. Add secrets on the Secrets page first.
              </p>
            )}
          </div>

          <div className="flex justify-end gap-2 mt-2">
            <PillButton variant="outlined" onClick={resetForm}>
              Cancel
            </PillButton>
            <PillButton
              variant="brand"
              onClick={handleSave}
              disabled={
                !name ||
                !command ||
                saving ||
                (runMode === "DockerSandbox" && !dockerImage.trim())
              }
            >
              {saving
                ? "Saving..."
                : editingServer
                  ? "Update Server"
                  : "Save Server"}
            </PillButton>
          </div>
        </div>
      </Modal>

      <RegistryBrowser
        open={showRegistry}
        onClose={() => setShowRegistry(false)}
        onInstall={handleInstallFromRegistry}
      />

      <Modal
        open={pendingSave}
        onClose={() => setPendingSave(false)}
        title="Untrusted Server Warning"
        size="md"
      >
        <div className="flex flex-col gap-4">
          <div className="rounded-lg border border-warning/30 bg-bg-elevated p-3 flex items-start gap-2">
            <ShieldAlert size={16} className="text-warning mt-0.5 shrink-0" />
            <div>
              <p className="text-sm font-bold text-text-primary">
                This server is not verified
              </p>
              <p className="text-xs text-text-secondary mt-1">
                If an AI client launches this server, it will run with access to
                the secrets mapped below. Only continue if you trust the command
                and its package source.
              </p>
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <p className="text-sm font-bold text-text-primary">Server</p>
            <div className="rounded-lg border border-border-default/30 bg-bg-elevated px-3 py-2">
              <p className="text-sm text-text-primary font-bold">{name}</p>
              <p className="text-xs text-text-secondary font-mono mt-1 break-all">
                {command} {args}
              </p>
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <p className="text-sm font-bold text-text-primary">
              Secrets this server can access
            </p>
            {envMappings.filter((m) => m.env_var_name.trim() && m.secret_ref.trim())
              .length === 0 ? (
              <p className="text-xs text-text-secondary">
                No secrets mapped yet.
              </p>
            ) : (
              <ul className="flex flex-col gap-1">
                {envMappings
                  .filter((m) => m.env_var_name.trim() && m.secret_ref.trim())
                  .map((mapping) => {
                    const secret = secrets.find((s) => s.id === mapping.secret_ref);
                    return (
                      <li
                        key={`${mapping.env_var_name}:${mapping.secret_ref}`}
                        className="text-xs text-text-secondary"
                      >
                        <code className="text-text-bright font-bold">
                          {mapping.env_var_name}
                        </code>
                        <span className="mx-2">→</span>
                        <span>{secret?.label ?? mapping.secret_ref}</span>
                      </li>
                    );
                  })}
              </ul>
            )}
          </div>

          <div className="flex justify-end gap-2">
            <PillButton variant="outlined" onClick={() => setPendingSave(false)}>
              Cancel
            </PillButton>
            <PillButton
              variant="brand"
              onClick={async () => {
                setPendingSave(false);
                await persistServer();
              }}
              disabled={saving}
            >
              {saving ? "Saving..." : "Continue Untrusted"}
            </PillButton>
          </div>
        </div>
      </Modal>
    </MainContent>
  );
}
