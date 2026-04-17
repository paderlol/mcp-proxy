import { useEffect } from "react";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { Badge } from "../components/ui/Badge";
import { Server, KeyRound, Activity, Shield } from "lucide-react";
import { useServers } from "../hooks/useServers";
import { useSecrets } from "../hooks/useSecrets";

export function Dashboard() {
  const { servers, proxyStatuses, fetchServers } = useServers();
  const { secrets, fetchSecrets } = useSecrets();

  useEffect(() => {
    fetchServers();
    fetchSecrets();
  }, [fetchServers, fetchSecrets]);

  const runningCount = Object.values(proxyStatuses).filter(
    (s) => s.running,
  ).length;

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
      label: "Active Proxies",
      value: String(runningCount),
      icon: Activity,
      color: "text-warning",
    },
    {
      label: "Keychain Status",
      value: "OK",
      icon: Shield,
      color: "text-brand",
    },
  ];

  return (
    <MainContent
      title="Dashboard"
      description="Overview of your MCP proxy configuration"
    >
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
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
            {servers.map((s) => {
              const status = proxyStatuses[s.id];
              return (
                <Card key={s.id}>
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-sm font-bold text-text-primary">
                        {s.name}
                      </p>
                      <p className="text-xs text-text-secondary font-mono">
                        {s.command} {s.args.join(" ")}
                      </p>
                    </div>
                    <Badge variant={status?.running ? "success" : "default"}>
                      {status?.running ? "Running" : "Stopped"}
                    </Badge>
                  </div>
                </Card>
              );
            })}
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
                  Generate config for Claude, Codex, Cursor, VS Code, or
                  Windsurf. Secrets never appear in config files.
                </p>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </MainContent>
  );
}
