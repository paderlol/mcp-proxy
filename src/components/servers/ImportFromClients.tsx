import { useEffect, useMemo, useState } from "react";
import { Modal } from "../ui/Modal";
import { PillButton } from "../ui/PillButton";
import { Badge } from "../ui/Badge";
import { KeyRound, ShieldAlert, ShieldCheck, Eye, EyeOff } from "lucide-react";
import { discoverClientServers, importServers } from "../../lib/tauri";
import type {
  DiscoveredServer,
  EnvDecision,
  ImportSelection,
  SourceClient,
} from "../../lib/types";

type Step = "discover" | "env" | "confirm";

interface Props {
  open: boolean;
  onClose: () => void;
  onImported?: (createdServerIds: string[]) => void;
}

type EnvKind = "secret" | "plaintext";

interface EnvState {
  kind: EnvKind;
  secretId: string;
  label: string;
  value: string;
  reveal: boolean;
}

const SECRET_PATTERN = /(TOKEN|KEY|SECRET|PASSWORD|CREDENTIAL)$/i;

const sourceLabel: Record<SourceClient, string> = {
  ClaudeDesktop: "Claude Desktop",
  ClaudeCode: "Claude Code",
  Codex: "Codex",
  Cursor: "Cursor",
  VsCode: "VS Code",
  Windsurf: "Windsurf",
};

function defaultSecretId(serverName: string, envName: string): string {
  const slug = serverName
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return `${slug}_${envName.toLowerCase()}`;
}

function discoveredKey(d: DiscoveredServer): string {
  return `${d.source}::${d.source_path}::${d.name}`;
}

export function ImportFromClients({ open, onClose, onImported }: Props) {
  const [step, setStep] = useState<Step>("discover");
  const [discovered, setDiscovered] = useState<DiscoveredServer[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [trustFlags, setTrustFlags] = useState<Record<string, boolean>>({});
  // keyed by `${discoveredKey}::${envName}`
  const [envState, setEnvState] = useState<Record<string, EnvState>>({});
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) return;
    setStep("discover");
    setSelected(new Set());
    setTrustFlags({});
    setEnvState({});
    setError(null);
    setLoading(true);
    discoverClientServers()
      .then((rows) => setDiscovered(rows))
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [open]);

  const selectedServers = useMemo(
    () => discovered.filter((d) => selected.has(discoveredKey(d))),
    [discovered, selected],
  );

  const toggleSelect = (d: DiscoveredServer) => {
    const key = discoveredKey(d);
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const seedEnvState = () => {
    const next: Record<string, EnvState> = { ...envState };
    for (const d of selectedServers) {
      for (const [name, value] of Object.entries(d.env)) {
        const k = `${discoveredKey(d)}::${name}`;
        if (next[k]) continue;
        const looksSecret = SECRET_PATTERN.test(name);
        next[k] = {
          kind: looksSecret ? "secret" : "plaintext",
          secretId: defaultSecretId(d.name, name),
          label: name,
          value,
          reveal: false,
        };
      }
    }
    setEnvState(next);
  };

  const updateEnv = (key: string, patch: Partial<EnvState>) => {
    setEnvState((prev) => ({ ...prev, [key]: { ...prev[key], ...patch } }));
  };

  const goNext = () => {
    if (step === "discover") {
      if (selected.size === 0) return;
      seedEnvState();
      setStep("env");
    } else if (step === "env") {
      setStep("confirm");
    }
  };

  const goBack = () => {
    if (step === "env") setStep("discover");
    else if (step === "confirm") setStep("env");
  };

  const submit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const selections: ImportSelection[] = selectedServers.map((d) => {
        const env_decisions: EnvDecision[] = Object.entries(d.env).map(
          ([name, value]) => {
            const s = envState[`${discoveredKey(d)}::${name}`];
            if (s && s.kind === "secret") {
              return {
                kind: "Secret",
                env_var_name: name,
                secret_id: s.secretId.trim() || defaultSecretId(d.name, name),
                label: s.label.trim() || name,
                value: s.value,
              };
            }
            return {
              kind: "Plaintext",
              env_var_name: name,
              value: s?.value ?? value,
            };
          },
        );
        return {
          discovered: d,
          env_decisions,
          trusted: trustFlags[discoveredKey(d)] ?? false,
        };
      });
      const results = await importServers(selections);
      onImported?.(results.map((r) => r.server_id));
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const title =
    step === "discover"
      ? "Import from AI Clients — Pick Servers"
      : step === "env"
        ? "Import — Configure Environment"
        : "Import — Confirm";

  return (
    <Modal open={open} onClose={onClose} title={title} size="xl">
      {error && (
        <div className="mb-3 p-3 rounded-md bg-red-500/10 text-red-400 text-sm">
          {error}
        </div>
      )}

      {step === "discover" && (
        <DiscoverStep
          loading={loading}
          discovered={discovered}
          selected={selected}
          onToggle={toggleSelect}
        />
      )}

      {step === "env" && (
        <EnvStep
          servers={selectedServers}
          envState={envState}
          updateEnv={updateEnv}
          trustFlags={trustFlags}
          setTrusted={(key, v) =>
            setTrustFlags((prev) => ({ ...prev, [key]: v }))
          }
        />
      )}

      {step === "confirm" && (
        <ConfirmStep
          servers={selectedServers}
          envState={envState}
          trustFlags={trustFlags}
        />
      )}

      <div className="flex justify-between gap-2 mt-5 pt-4 border-t border-border-default/30">
        <PillButton variant="outlined" onClick={onClose}>
          Cancel
        </PillButton>
        <div className="flex gap-2">
          {step !== "discover" && (
            <PillButton variant="outlined" onClick={goBack}>
              Back
            </PillButton>
          )}
          {step !== "confirm" ? (
            <PillButton
              variant="brand"
              onClick={goNext}
              disabled={step === "discover" && selected.size === 0}
            >
              Next
            </PillButton>
          ) : (
            <PillButton variant="brand" onClick={submit} disabled={submitting}>
              {submitting ? "Importing…" : `Import ${selectedServers.length}`}
            </PillButton>
          )}
        </div>
      </div>
    </Modal>
  );
}

function DiscoverStep({
  loading,
  discovered,
  selected,
  onToggle,
}: {
  loading: boolean;
  discovered: DiscoveredServer[];
  selected: Set<string>;
  onToggle: (d: DiscoveredServer) => void;
}) {
  if (loading) {
    return (
      <p className="text-sm text-text-secondary py-6 text-center">
        Scanning AI client configs…
      </p>
    );
  }
  if (discovered.length === 0) {
    return (
      <p className="text-sm text-text-secondary py-6 text-center">
        No MCP servers found in Claude Desktop, Claude Code, Codex, Cursor, VS
        Code, or Windsurf config files.
      </p>
    );
  }

  const bySource = discovered.reduce<Record<string, DiscoveredServer[]>>(
    (acc, d) => {
      (acc[d.source] ||= []).push(d);
      return acc;
    },
    {},
  );

  return (
    <div className="flex flex-col gap-4 max-h-[55vh] overflow-y-auto">
      {Object.entries(bySource).map(([source, servers]) => (
        <div key={source}>
          <h3 className="text-xs font-bold text-text-secondary uppercase tracking-wider mb-2">
            {sourceLabel[source as SourceClient] ?? source}
          </h3>
          <ul className="flex flex-col gap-1.5">
            {servers.map((d) => {
              const key = discoveredKey(d);
              const checked = selected.has(key);
              const envCount = Object.keys(d.env).length;
              return (
                <li
                  key={key}
                  onClick={() => onToggle(d)}
                  className={`p-3 rounded-md border cursor-pointer transition-colors ${
                    checked
                      ? "border-brand/60 bg-brand/5"
                      : "border-border-default/30 bg-bg-elevated hover:border-border-default/60"
                  }`}
                >
                  <div className="flex items-start gap-3">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => onToggle(d)}
                      className="mt-1 accent-brand"
                      onClick={(e) => e.stopPropagation()}
                    />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <p className="text-sm font-bold text-text-primary">
                          {d.name}
                        </p>
                        <Badge>{d.transport}</Badge>
                        {envCount > 0 && (
                          <Badge>
                            <KeyRound size={10} className="mr-1" />
                            {envCount} env
                          </Badge>
                        )}
                      </div>
                      <p className="text-xs text-text-secondary font-mono mt-1 truncate">
                        {d.command} {d.args.join(" ")}
                      </p>
                      <p className="text-[10px] text-text-secondary/60 mt-0.5 truncate">
                        {d.source_path}
                      </p>
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </div>
  );
}

function EnvStep({
  servers,
  envState,
  updateEnv,
  trustFlags,
  setTrusted,
}: {
  servers: DiscoveredServer[];
  envState: Record<string, EnvState>;
  updateEnv: (key: string, patch: Partial<EnvState>) => void;
  trustFlags: Record<string, boolean>;
  setTrusted: (key: string, v: boolean) => void;
}) {
  return (
    <div className="flex flex-col gap-5 max-h-[60vh] overflow-y-auto">
      {servers.map((d) => {
        const envs = Object.entries(d.env);
        const srvKey = discoveredKey(d);
        const trusted = trustFlags[srvKey] ?? false;
        return (
          <div
            key={srvKey}
            className="p-3 rounded-md border border-border-default/30 bg-bg-elevated"
          >
            <div className="flex items-center justify-between mb-2 flex-wrap gap-2">
              <div>
                <p className="text-sm font-bold text-text-primary">{d.name}</p>
                <p className="text-[11px] text-text-secondary/70">
                  from {sourceLabel[d.source] ?? d.source}
                </p>
              </div>
              <div className="flex gap-1">
                <button
                  type="button"
                  onClick={() => setTrusted(srvKey, false)}
                  className={`px-2.5 py-1 text-xs rounded-full border ${
                    !trusted
                      ? "border-warning/60 bg-warning/10 text-warning"
                      : "border-border-default/40 text-text-secondary"
                  }`}
                >
                  <ShieldAlert size={11} className="inline mr-1" />
                  Untrusted
                </button>
                <button
                  type="button"
                  onClick={() => setTrusted(srvKey, true)}
                  className={`px-2.5 py-1 text-xs rounded-full border ${
                    trusted
                      ? "border-brand/60 bg-brand/10 text-brand"
                      : "border-border-default/40 text-text-secondary"
                  }`}
                >
                  <ShieldCheck size={11} className="inline mr-1" />
                  Trusted
                </button>
              </div>
            </div>
            {envs.length === 0 ? (
              <p className="text-xs text-text-secondary/60 italic">
                No environment variables.
              </p>
            ) : (
              <table className="w-full text-xs">
                <thead className="text-text-secondary">
                  <tr>
                    <th className="text-left py-1 font-medium">Name</th>
                    <th className="text-left py-1 font-medium">Value</th>
                    <th className="text-left py-1 font-medium">Storage</th>
                    <th className="text-left py-1 font-medium">Secret ID</th>
                  </tr>
                </thead>
                <tbody>
                  {envs.map(([envName, value]) => {
                    const k = `${srvKey}::${envName}`;
                    const s =
                      envState[k] ??
                      ({
                        kind: SECRET_PATTERN.test(envName)
                          ? "secret"
                          : "plaintext",
                        secretId: defaultSecretId(d.name, envName),
                        label: envName,
                        value,
                        reveal: false,
                      } as EnvState);
                    const looksSecret = SECRET_PATTERN.test(envName);
                    return (
                      <tr
                        key={envName}
                        className="border-t border-border-default/20"
                      >
                        <td className="py-1.5 pr-2">
                          <code className="text-text-primary font-mono">
                            {envName}
                          </code>
                          {looksSecret && (
                            <span className="ml-1 text-[10px] text-warning">
                              likely secret
                            </span>
                          )}
                        </td>
                        <td className="py-1.5 pr-2">
                          <div className="flex items-center gap-1">
                            <input
                              type={s.reveal ? "text" : "password"}
                              value={s.value}
                              onChange={(e) =>
                                updateEnv(k, { value: e.target.value })
                              }
                              className="bg-bg-base text-text-primary text-xs px-2 py-1 rounded border border-border-default/40 w-36 font-mono"
                            />
                            <button
                              type="button"
                              onClick={() => updateEnv(k, { reveal: !s.reveal })}
                              className="text-text-secondary hover:text-text-primary"
                            >
                              {s.reveal ? (
                                <EyeOff size={12} />
                              ) : (
                                <Eye size={12} />
                              )}
                            </button>
                          </div>
                        </td>
                        <td className="py-1.5 pr-2">
                          <div className="flex gap-1">
                            <button
                              type="button"
                              onClick={() => updateEnv(k, { kind: "secret" })}
                              className={`px-2 py-0.5 rounded-full text-[10px] border ${
                                s.kind === "secret"
                                  ? "border-brand/60 bg-brand/10 text-brand"
                                  : "border-border-default/40 text-text-secondary"
                              }`}
                            >
                              Secret
                            </button>
                            <button
                              type="button"
                              onClick={() =>
                                updateEnv(k, { kind: "plaintext" })
                              }
                              className={`px-2 py-0.5 rounded-full text-[10px] border ${
                                s.kind === "plaintext"
                                  ? "border-border-default/70 bg-bg-base text-text-primary"
                                  : "border-border-default/40 text-text-secondary"
                              }`}
                            >
                              Plaintext
                            </button>
                          </div>
                        </td>
                        <td className="py-1.5">
                          {s.kind === "secret" ? (
                            <input
                              type="text"
                              value={s.secretId}
                              onChange={(e) =>
                                updateEnv(k, { secretId: e.target.value })
                              }
                              className="bg-bg-base text-text-primary text-xs px-2 py-1 rounded border border-border-default/40 w-40 font-mono"
                            />
                          ) : (
                            <span className="text-text-secondary/50 italic">
                              —
                            </span>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        );
      })}
    </div>
  );
}

function ConfirmStep({
  servers,
  envState,
  trustFlags,
}: {
  servers: DiscoveredServer[];
  envState: Record<string, EnvState>;
  trustFlags: Record<string, boolean>;
}) {
  return (
    <div className="flex flex-col gap-3 max-h-[60vh] overflow-y-auto">
      <p className="text-sm text-text-secondary">
        {servers.length} server{servers.length === 1 ? "" : "s"} will be
        imported. Secrets are written to your local vault; plaintext values are
        saved inline.
      </p>
      {servers.map((d) => {
        const srvKey = discoveredKey(d);
        const trusted = trustFlags[srvKey] ?? false;
        return (
          <div
            key={srvKey}
            className="p-3 rounded-md border border-border-default/30 bg-bg-elevated"
          >
            <div className="flex items-center gap-2 mb-1 flex-wrap">
              <p className="text-sm font-bold text-text-primary">{d.name}</p>
              <Badge variant={trusted ? "success" : "warning"}>
                {trusted ? "Trusted" : "Untrusted"}
              </Badge>
              <span className="text-[11px] text-text-secondary/70">
                {sourceLabel[d.source] ?? d.source}
              </span>
            </div>
            <p className="text-xs text-text-secondary font-mono truncate">
              {d.command} {d.args.join(" ")}
            </p>
            {Object.entries(d.env).length > 0 && (
              <ul className="mt-2 flex flex-col gap-0.5">
                {Object.entries(d.env).map(([name]) => {
                  const s = envState[`${srvKey}::${name}`];
                  return (
                    <li key={name} className="text-[11px] text-text-secondary">
                      <code className="text-text-bright">{name}</code>
                      <span className="mx-1.5">→</span>
                      {s?.kind === "secret" ? (
                        <>
                          vault secret{" "}
                          <code className="text-brand">{s.secretId}</code>
                        </>
                      ) : (
                        <span className="text-text-secondary/80">
                          plaintext
                        </span>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        );
      })}
    </div>
  );
}
