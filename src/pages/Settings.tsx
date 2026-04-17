import { useEffect, useState } from "react";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { PillButton } from "../components/ui/PillButton";
import { Badge } from "../components/ui/Badge";
import { SecretInput } from "../components/ui/SecretInput";
import { Lock, Unlock, AlertTriangle } from "lucide-react";
import { useVault } from "../hooks/useVault";

export function Settings() {
  const { status, busy, error, refresh, unlock, lock } = useVault();
  const [password, setPassword] = useState("");

  useEffect(() => {
    refresh();
  }, [refresh]);

  const isKeychain = status?.backend === "keychain";
  const showVaultCard = status && !isKeychain;

  const localDescription = isKeychain
    ? "Hardware-backed encryption via the system Keychain — unlocks with your login."
    : "AES-256-GCM encrypted file, unlocked with a master password held only in memory.";

  const handleSubmitPassword = async () => {
    if (!password) return;
    try {
      await unlock(password);
      setPassword("");
    } catch {
      // error is surfaced via `error` state
    }
  };

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

        {/* Security summary */}
        <Card>
          <h3 className="text-sm font-bold text-text-primary mb-4">Security</h3>
          <div className="flex items-center justify-between py-2">
            <div>
              <p className="text-sm text-text-primary">Local Storage</p>
              <p className="text-xs text-text-secondary">
                {localDescription}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary">
                {isKeychain ? "macOS Keychain" : "AES-256 Vault"}
              </span>
              <Badge variant={isKeychain || status?.unlocked ? "success" : "warning"}>
                {isKeychain
                  ? "Always available"
                  : status?.unlocked
                    ? "Unlocked"
                    : status?.exists
                      ? "Locked"
                      : "Not created"}
              </Badge>
            </div>
          </div>
        </Card>

        {/* Vault control — only for encrypted-file backend */}
        {showVaultCard && (
          <Card>
            <h3 className="text-sm font-bold text-text-primary mb-1">
              Local Vault
            </h3>
            <p className="text-xs text-text-secondary mb-4">
              Secrets marked <strong>Local</strong> are encrypted with a master
              password. The derived key is held in memory until you lock it or
              close the app. To let the CLI unlock the vault at MCP-server
              launch, set the{" "}
              <code className="text-text-bright">MCP_PROXY_MASTER_PASSWORD</code>{" "}
              environment variable in the shell that launches your AI client.
            </p>

            {status?.unlocked ? (
              <div className="flex items-center justify-between">
                <p className="text-sm text-text-primary flex items-center gap-2">
                  <Unlock size={14} className="text-brand" />
                  Vault is unlocked
                </p>
                <PillButton
                  variant="outlined"
                  onClick={() => lock()}
                  disabled={busy}
                >
                  <Lock size={12} className="mr-1" />
                  Lock
                </PillButton>
              </div>
            ) : (
              <div className="flex flex-col gap-3">
                <SecretInput
                  label={
                    status?.exists
                      ? "Master password"
                      : "Set a master password (vault will be created)"
                  }
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSubmitPassword();
                  }}
                  placeholder="•••••••••"
                />
                {error && (
                  <p className="text-xs text-negative flex items-center gap-1">
                    <AlertTriangle size={12} />
                    {error}
                  </p>
                )}
                <div className="flex justify-end">
                  <PillButton
                    variant="brand"
                    onClick={handleSubmitPassword}
                    disabled={busy || !password}
                  >
                    <Unlock size={12} className="mr-1" />
                    {status?.exists ? "Unlock" : "Create Vault"}
                  </PillButton>
                </div>
              </div>
            )}
          </Card>
        )}

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
              <span className="text-text-secondary">Backend</span>
              <span className="text-text-primary">
                {status?.backend ?? "…"}
              </span>
            </div>
          </div>
        </Card>
      </div>
    </MainContent>
  );
}
