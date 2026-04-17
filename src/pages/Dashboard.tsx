import { useEffect, useState } from "react";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { Badge } from "../components/ui/Badge";
import { Server, KeyRound, Shield, RefreshCw } from "lucide-react";
import { useServers } from "../hooks/useServers";
import { useSecrets } from "../hooks/useSecrets";
import { useVault } from "../hooks/useVault";

/**
 * Refresh vault status this often while the Dashboard is mounted. Vault
 * state can change without this tab's knowledge (another component locks,
 * an auto-lock timer fires, the user re-opens the app after unlocking
 * elsewhere) so a gentle poll keeps the status card honest.
 */
const VAULT_POLL_INTERVAL_MS = 5_000;

function formatTime(ms: number): string {
  const d = new Date(ms);
  const hh = d.getHours().toString().padStart(2, "0");
  const mm = d.getMinutes().toString().padStart(2, "0");
  const ss = d.getSeconds().toString().padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

export function Dashboard() {
  const { servers, fetchServers } = useServers();
  const { secrets, fetchSecrets } = useSecrets();
  const { status: vaultStatus, refresh: refreshVault } = useVault();
  const [lastRefresh, setLastRefresh] = useState<number>(() => Date.now());

  // Initial fetch on mount.
  useEffect(() => {
    fetchServers();
    fetchSecrets();
    refreshVault();
    setLastRefresh(Date.now());
  }, [fetchServers, fetchSecrets, refreshVault]);

  // Live poll on vault status while mounted. Server/secret counts only change
  // via explicit user action elsewhere, so we don't waste work polling them.
  useEffect(() => {
    const id = setInterval(async () => {
      await refreshVault();
      setLastRefresh(Date.now());
    }, VAULT_POLL_INTERVAL_MS);
    return () => clearInterval(id);
  }, [refreshVault]);

  // Pick a sensible label for the secret-storage status card.
  const storageLabel = vaultStatus?.backend === "keychain"
    ? "Ready"
    : vaultStatus?.unlocked
      ? "Unlocked"
      : vaultStatus?.exists
        ? "Locked"
        : "Not set up";
  const storageColor =
    vaultStatus?.backend === "keychain" || vaultStatus?.unlocked
      ? "text-brand"
      : "text-warning";

  const stats = [
    {
      label: "MCP Servers",
      value: String(servers.length),
      icon: Server,
      color: "text-brand",
    },
    {
      label: "Secrets Stored",
      value: String(secrets.length),
      icon: KeyRound,
      color: "text-info",
    },
    {
      label:
        vaultStatus?.backend === "keychain" ? "Keychain" : "Local Vault",
      value: storageLabel,
      icon: Shield,
      color: storageColor,
    },
  ];

  return (
    <MainContent
      title="Dashboard"
      description="Overview of your MCP proxy configuration"
      actions={
        <div className="flex items-center gap-1.5 text-xs text-text-secondary">
          <RefreshCw size={12} className="animate-[spin_6s_linear_infinite] opacity-60" />
          <span>Live · last updated {formatTime(lastRefresh)}</span>
        </div>
      }
    >
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
        {stats.map((stat) => (
          <Card key={stat.label}>
            <div className="flex items-center gap-3">
              <div className={`p-2.5 rounded-lg bg-bg-elevated ${stat.color}`}>
                <stat.icon size={18} />
              </div>
              <div>
                <p className="text-2xl font-bold text-text-primary">
                  {stat.value}
                </p>
                <p className="text-xs text-text-secondary">{stat.label}</p>
              </div>
            </div>
          </Card>
        ))}
      </div>

      <div className="mb-8">
        <h2 className="text-base font-bold text-text-primary mb-4">
          Configured Servers
        </h2>
        {servers.length === 0 ? (
          <Card>
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <Server size={40} className="text-text-secondary/40 mb-3" />
              <p className="text-sm text-text-secondary mb-1">
                No MCP servers configured yet
              </p>
              <p className="text-xs text-text-secondary/60">
                Go to Servers to add your first MCP server configuration
              </p>
            </div>
          </Card>
        ) : (
          <div className="flex flex-col gap-2">
            {servers.map((s) => (
              <Card key={s.id}>
                <div className="flex items-center justify-between">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-0.5 flex-wrap">
                      <p className="text-sm font-bold text-text-primary">
                        {s.name}
                      </p>
                      <Badge>
                        {s.run_mode.type === "Local" ? "Local" : "Docker"}
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
                </div>
              </Card>
            ))}
          </div>
        )}
      </div>

      <div>
        <h2 className="text-base font-bold text-text-primary mb-4">
          How It Works
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <Card>
            <div className="flex items-start gap-3">
              <Badge variant="success">1</Badge>
              <div>
                <p className="text-sm font-bold text-text-primary mb-1">
                  Store Secrets
                </p>
                <p className="text-xs text-text-secondary">
                  Add your API keys and tokens — stored locally on this device,
                  or referenced from 1Password.
                </p>
              </div>
            </div>
          </Card>
          <Card>
            <div className="flex items-start gap-3">
              <Badge variant="success">2</Badge>
              <div>
                <p className="text-sm font-bold text-text-primary mb-1">
                  Configure Servers
                </p>
                <p className="text-xs text-text-secondary">
                  Set up MCP servers and map secrets to environment variables.
                </p>
              </div>
            </div>
          </Card>
          <Card>
            <div className="flex items-start gap-3">
              <Badge variant="success">3</Badge>
              <div>
                <p className="text-sm font-bold text-text-primary mb-1">
                  Generate Config
                </p>
                <p className="text-xs text-text-secondary">
                  Write config straight to Claude, Codex, Cursor, VS Code, or
                  Windsurf. Secrets never appear in config files; they are
                  resolved at runtime by <code className="text-text-bright">mcp-proxy run</code>.
                </p>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </MainContent>
  );
}
