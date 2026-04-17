import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { PillButton } from "../components/ui/PillButton";
import { Badge } from "../components/ui/Badge";

function getLocalBackendInfo() {
  const platform =
    typeof navigator !== "undefined" ? navigator.platform || "" : "";
  const isMac = /Mac|iPhone|iPad/.test(platform);
  return {
    label: isMac ? "macOS Keychain" : "AES-256 Vault",
    description: isMac
      ? "Hardware-backed encryption via the system Keychain"
      : "Local encrypted vault (not yet implemented — use 1Password for now)",
    status: isMac ? "Connected" : "Pending",
    variant: isMac ? ("success" as const) : ("warning" as const),
  };
}

export function Settings() {
  const backend = getLocalBackendInfo();
  return (
    <MainContent
      title="Settings"
      description="Application preferences and system info"
    >
      <div className="flex flex-col gap-6">
        {/* General */}
        <Card>
          <h3 className="text-sm font-bold text-text-primary mb-4">General</h3>
          <div className="flex items-center justify-between py-2">
            <div>
              <p className="text-sm text-text-primary">Launch at Login</p>
              <p className="text-xs text-text-secondary">
                Start MCP Proxy automatically when you log in
              </p>
            </div>
            <PillButton variant="outlined">Enable</PillButton>
          </div>
        </Card>

        {/* Security */}
        <Card>
          <h3 className="text-sm font-bold text-text-primary mb-4">
            Security
          </h3>
          <div className="flex items-center justify-between py-2">
            <div>
              <p className="text-sm text-text-primary">Local Storage</p>
              <p className="text-xs text-text-secondary">
                {backend.description}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary">
                {backend.label}
              </span>
              <Badge variant={backend.variant}>{backend.status}</Badge>
            </div>
          </div>
        </Card>

        {/* About */}
        <Card>
          <h3 className="text-sm font-bold text-text-primary mb-4">About</h3>
          <div className="flex flex-col gap-2">
            <div className="flex justify-between text-sm">
              <span className="text-text-secondary">Version</span>
              <span className="text-text-primary">0.1.0</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-text-secondary">Runtime</span>
              <span className="text-text-primary">Tauri v2</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-text-secondary">Platform</span>
              <span className="text-text-primary">macOS (aarch64)</span>
            </div>
          </div>
        </Card>
      </div>
    </MainContent>
  );
}
