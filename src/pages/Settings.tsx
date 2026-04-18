import { useEffect, useState } from "react";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { PillButton } from "../components/ui/PillButton";
import { Badge } from "../components/ui/Badge";
import { Modal } from "../components/ui/Modal";
import { SecretInput } from "../components/ui/SecretInput";
import {
  Lock,
  Unlock,
  AlertTriangle,
  KeyRound,
  Trash2,
  Clock,
  RefreshCw,
  FileClock,
  Shuffle,
} from "lucide-react";
import { useVault } from "../hooks/useVault";
import {
  IDLE_TIMEOUT_CHOICES,
  useVaultIdleTimeout,
} from "../hooks/useVaultIdleTimeout";
import { useAutostart } from "../hooks/useAutostart";
import { listAuditLogs } from "../lib/tauri";
import type { AuditLogEntry } from "../lib/types";

export function Settings() {
  const {
    status,
    busy,
    error,
    refresh,
    unlock,
    lock,
    changePassword,
    reset,
    setPreferLocalVault,
  } = useVault();
  const [idleTimeoutMs, setIdleTimeoutMs] = useVaultIdleTimeout();
  const autostart = useAutostart();

  // unlock / create
  const [password, setPassword] = useState("");

  // change password
  const [showChangePw, setShowChangePw] = useState(false);
  const [newPw1, setNewPw1] = useState("");
  const [newPw2, setNewPw2] = useState("");
  const [changePwBusy, setChangePwBusy] = useState(false);
  const [changePwError, setChangePwError] = useState<string | null>(null);
  const [changePwSuccess, setChangePwSuccess] = useState(false);

  // reset confirmation
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [resetConfirmText, setResetConfirmText] = useState("");

  // backend switch confirmation (macOS-only)
  const [showBackendSwitch, setShowBackendSwitch] = useState(false);
  const [backendSwitchError, setBackendSwitchError] = useState<string | null>(
    null,
  );
  const [auditLogs, setAuditLogs] = useState<AuditLogEntry[]>([]);
  const [auditBusy, setAuditBusy] = useState(false);
  const [auditError, setAuditError] = useState<string | null>(null);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    void refreshAuditLogs();
  }, []);

  const isKeychain = status?.backend === "keychain";
  const showVaultCard = status && !isKeychain;
  const canSwitchBackend = status?.can_switch_backend ?? false;

  const localDescription = isKeychain
    ? "Hardware-backed encryption via the system Keychain — unlocks with your login."
    : "AES-256-GCM encrypted file, unlocked with a master password held only in memory.";

  // Switching Vault → Keychain while the vault is locked would orphan any
  // encrypted secrets, so we disable that flip until the user unlocks.
  const switchToKeychainBlockedByLock =
    !isKeychain && status?.exists === true && status?.unlocked !== true;
  const nextBackendLabel = isKeychain ? "Local Vault" : "macOS Keychain";
  const switchButtonLabel = `Switch to ${nextBackendLabel}`;

  const handleSubmitPassword = async () => {
    if (!password) return;
    try {
      await unlock(password);
      setPassword("");
    } catch {
      /* error is surfaced via `error` state */
    }
  };

  const closeChangePw = () => {
    setShowChangePw(false);
    setNewPw1("");
    setNewPw2("");
    setChangePwError(null);
    setChangePwSuccess(false);
  };

  const submitChangePw = async () => {
    setChangePwError(null);
    if (newPw1.length < 8) {
      setChangePwError("New password must be at least 8 characters.");
      return;
    }
    if (newPw1 !== newPw2) {
      setChangePwError("The two new-password fields don't match.");
      return;
    }
    setChangePwBusy(true);
    try {
      await changePassword(newPw1);
      setChangePwSuccess(true);
      setNewPw1("");
      setNewPw2("");
    } catch (e) {
      setChangePwError(String(e));
    } finally {
      setChangePwBusy(false);
    }
  };

  const confirmBackendSwitch = async () => {
    setBackendSwitchError(null);
    try {
      // If currently on Keychain → flip to vault (prefer_local_vault = true).
      // If currently on Vault → flip back (prefer_local_vault = false).
      await setPreferLocalVault(isKeychain);
      setShowBackendSwitch(false);
    } catch (e) {
      setBackendSwitchError(String(e));
    }
  };

  const submitReset = async () => {
    try {
      await reset();
      setShowResetConfirm(false);
      setResetConfirmText("");
    } catch {
      /* surfaced via error state */
    }
  };

  const inputClass =
    "w-full bg-bg-elevated text-text-primary rounded-[500px] px-4 py-2.5 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] placeholder:text-text-secondary/50";

  async function refreshAuditLogs() {
    setAuditBusy(true);
    setAuditError(null);
    try {
      const entries = await listAuditLogs(50);
      setAuditLogs(entries);
    } catch (err) {
      setAuditError(String(err));
    } finally {
      setAuditBusy(false);
    }
  }

  const formatAuditTime = (iso: string) =>
    new Date(iso).toLocaleString(undefined, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });

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
                Start MCP Proxy automatically when you log in. On macOS this
                creates a LaunchAgent; on Linux an autostart .desktop entry;
                on Windows a `Run` registry key.
              </p>
            </div>
            {autostart.supported ? (
              <div className="flex items-center gap-2">
                <Badge
                  variant={autostart.enabled ? "success" : "default"}
                >
                  {autostart.enabled === null
                    ? "…"
                    : autostart.enabled
                      ? "On"
                      : "Off"}
                </Badge>
                <PillButton
                  variant="outlined"
                  onClick={() => autostart.toggle()}
                  disabled={autostart.busy || autostart.enabled === null}
                >
                  {autostart.enabled ? "Disable" : "Enable"}
                </PillButton>
              </div>
            ) : (
              <Badge variant="default">Unavailable</Badge>
            )}
          </div>
        </Card>

        {/* Security summary */}
        <Card>
          <h3 className="text-sm font-bold text-text-primary mb-4">Security</h3>
          <div className="flex items-center justify-between py-2">
            <div>
              <p className="text-sm text-text-primary">Local Storage</p>
              <p className="text-xs text-text-secondary">{localDescription}</p>
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

          {canSwitchBackend && (
            <div className="border-t border-border-default/30 pt-3 mt-3 flex items-center justify-between gap-3">
              <div>
                <p className="text-sm text-text-primary flex items-center gap-2">
                  <Shuffle size={14} className="text-info" />
                  Change Local backend
                </p>
                <p className="text-xs text-text-secondary">
                  {isKeychain
                    ? "Opt into the AES-256 encrypted vault instead of the system Keychain. Switching does not migrate existing secrets."
                    : "Go back to using the macOS Keychain. Unlock the vault first so nothing encrypted is orphaned. Switching does not migrate existing secrets."}
                </p>
                {switchToKeychainBlockedByLock && (
                  <p className="text-xs text-warning mt-1 flex items-center gap-1">
                    <AlertTriangle size={12} />
                    Unlock the vault before switching back to Keychain.
                  </p>
                )}
              </div>
              <PillButton
                variant="brand"
                onClick={() => {
                  setBackendSwitchError(null);
                  setShowBackendSwitch(true);
                }}
                disabled={busy || switchToKeychainBlockedByLock}
                aria-label={switchButtonLabel}
              >
                <Shuffle size={12} className="mr-1" />
                {switchButtonLabel}
              </PillButton>
            </div>
          )}
        </Card>

        <Card>
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-sm font-bold text-text-primary">
                Audit Log
              </h3>
              <p className="text-xs text-text-secondary mt-1">
                Recent secret-resolution activity recorded by the CLI. Secret
                values are never stored here.
              </p>
            </div>
            <PillButton
              variant="outlined"
              onClick={() => void refreshAuditLogs()}
              disabled={auditBusy}
            >
              <RefreshCw size={12} className="mr-1" />
              {auditBusy ? "Refreshing..." : "Refresh"}
            </PillButton>
          </div>

          {auditError ? (
            <p className="text-xs text-negative flex items-center gap-1">
              <AlertTriangle size={12} />
              {auditError}
            </p>
          ) : auditLogs.length === 0 ? (
            <div className="flex items-center gap-3 py-4 text-text-secondary">
              <FileClock size={18} className="text-text-secondary/60" />
              <p className="text-sm">
                No audit log entries yet. Entries appear after an AI client
                launches <code className="text-text-bright">mcp-proxy run</code>.
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              {auditLogs.map((entry, index) => (
                <div
                  key={`${entry.timestamp}:${entry.server_id}:${entry.secret_id}:${index}`}
                  className="rounded-lg border border-border-default/30 bg-bg-elevated p-3"
                >
                  <div className="flex items-center justify-between gap-3 mb-1 flex-wrap">
                    <p className="text-sm font-bold text-text-primary">
                      {entry.server_id}
                    </p>
                    <Badge
                      variant={
                        entry.status.type === "Success" ? "success" : "warning"
                      }
                    >
                      {entry.status.type === "Success" ? "Success" : "Error"}
                    </Badge>
                  </div>
                  <p className="text-xs text-text-secondary">
                    Secret:{" "}
                    <code className="text-text-bright">{entry.secret_id}</code>
                    <span className="mx-2">·</span>
                    Source: {entry.source}
                  </p>
                  <p className="text-xs text-text-secondary mt-1">
                    {formatAuditTime(entry.timestamp)}
                  </p>
                  {entry.status.type === "Error" && (
                    <p className="text-xs text-warning mt-2">
                      {entry.status.message}
                    </p>
                  )}
                </div>
              ))}
            </div>
          )}
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
              close the app. Unlocking in the desktop app also writes a
              short-lived local session file so CLI launches can reuse the
              derived key without re-prompting. You can still fall back to the{" "}
              <code className="text-text-bright">MCP_PROXY_MASTER_PASSWORD</code>{" "}
              environment variable in the shell that launches your AI client.
            </p>

            {status?.unlocked ? (
              <div className="flex flex-col gap-4">
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

                <div className="border-t border-border-default/30 pt-4 flex items-center justify-between">
                  <div>
                    <p className="text-sm text-text-primary flex items-center gap-2">
                      <Clock size={14} className="text-info" />
                      Auto-lock after idle
                    </p>
                    <p className="text-xs text-text-secondary">
                      Automatically locks the vault after this much user inactivity.
                    </p>
                  </div>
                  <select
                    value={String(idleTimeoutMs)}
                    onChange={(e) => setIdleTimeoutMs(Number(e.target.value))}
                    className="bg-bg-elevated text-text-primary rounded-[500px] px-3 py-2 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] cursor-pointer"
                  >
                    {IDLE_TIMEOUT_CHOICES.map((c) => (
                      <option key={c.ms} value={String(c.ms)}>
                        {c.label}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="border-t border-border-default/30 pt-4 flex items-center justify-between">
                  <div>
                    <p className="text-sm text-text-primary flex items-center gap-2">
                      <KeyRound size={14} className="text-warning" />
                      Change master password
                    </p>
                    <p className="text-xs text-text-secondary">
                      Re-encrypts the vault. All existing secrets are preserved.
                    </p>
                  </div>
                  <PillButton
                    variant="outlined"
                    onClick={() => setShowChangePw(true)}
                    disabled={busy}
                  >
                    Change
                  </PillButton>
                </div>

                <div className="border-t border-border-default/30 pt-4 flex items-center justify-between">
                  <div>
                    <p className="text-sm text-text-primary flex items-center gap-2">
                      <Trash2 size={14} className="text-negative" />
                      Reset vault
                    </p>
                    <p className="text-xs text-text-secondary">
                      Deletes the vault file and all Local secrets. Irreversible.
                    </p>
                  </div>
                  <PillButton
                    variant="outlined"
                    onClick={() => setShowResetConfirm(true)}
                    className="!text-negative hover:!border-negative"
                    disabled={busy}
                  >
                    Reset…
                  </PillButton>
                </div>
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
                <div className="flex justify-end gap-2">
                  {status?.exists && (
                    <PillButton
                      variant="outlined"
                      onClick={() => setShowResetConfirm(true)}
                      className="!text-negative hover:!border-negative"
                      disabled={busy}
                    >
                      Reset…
                    </PillButton>
                  )}
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

      {/* Change password modal */}
      <Modal
        open={showChangePw}
        onClose={closeChangePw}
        title="Change master password"
      >
        <div className="flex flex-col gap-3">
          {changePwSuccess ? (
            <>
              <p className="text-sm text-text-primary">
                Vault re-encrypted with the new password.
              </p>
              <p className="text-xs text-text-secondary">
                Remember to update{" "}
                <code className="text-text-bright">MCP_PROXY_MASTER_PASSWORD</code>{" "}
                in the shell that launches your AI client, or future{" "}
                <code className="text-text-bright">mcp-proxy run</code>{" "}
                invocations will fail to unlock.
              </p>
              <div className="flex justify-end">
                <PillButton variant="brand" onClick={closeChangePw}>
                  Done
                </PillButton>
              </div>
            </>
          ) : (
            <>
              <SecretInput
                label="New master password"
                value={newPw1}
                onChange={(e) => setNewPw1(e.target.value)}
              />
              <SecretInput
                label="Confirm new password"
                value={newPw2}
                onChange={(e) => setNewPw2(e.target.value)}
              />
              {changePwError && (
                <p className="text-xs text-negative flex items-center gap-1">
                  <AlertTriangle size={12} />
                  {changePwError}
                </p>
              )}
              <div className="flex justify-end gap-2 mt-2">
                <PillButton variant="outlined" onClick={closeChangePw}>
                  Cancel
                </PillButton>
                <PillButton
                  variant="brand"
                  onClick={submitChangePw}
                  disabled={changePwBusy || !newPw1}
                >
                  {changePwBusy ? "Working..." : "Change Password"}
                </PillButton>
              </div>
            </>
          )}
        </div>
      </Modal>

      {/* Reset confirmation modal */}
      <Modal
        open={showResetConfirm}
        onClose={() => {
          setShowResetConfirm(false);
          setResetConfirmText("");
        }}
        title="Reset vault?"
      >
        <div className="flex flex-col gap-3">
          <div className="rounded-lg border border-negative/30 bg-bg-elevated p-3 flex items-start gap-2">
            <AlertTriangle
              size={14}
              className="text-negative flex-shrink-0 mt-0.5"
            />
            <p className="text-xs text-text-secondary leading-relaxed">
              This deletes <code className="text-text-bright">vault.bin</code>{" "}
              and every secret stored inside it. Secret metadata entries will
              still show in the list but will fail to resolve until you
              re-create them. There is no undo.
            </p>
          </div>
          <label className="text-xs text-text-secondary">
            Type{" "}
            <code className="text-text-bright font-bold">RESET</code> to confirm:
          </label>
          <input
            type="text"
            value={resetConfirmText}
            onChange={(e) => setResetConfirmText(e.target.value)}
            placeholder="RESET"
            className={`${inputClass} font-mono`}
          />
          <div className="flex justify-end gap-2 mt-2">
            <PillButton
              variant="outlined"
              onClick={() => {
                setShowResetConfirm(false);
                setResetConfirmText("");
              }}
            >
              Cancel
            </PillButton>
            <PillButton
              variant="brand"
              onClick={submitReset}
              disabled={resetConfirmText !== "RESET" || busy}
              className="!bg-negative !text-text-primary"
            >
              <Trash2 size={12} className="mr-1" />
              Reset Vault
            </PillButton>
          </div>
        </div>
      </Modal>

      {/* Backend switch confirmation modal */}
      <Modal
        open={showBackendSwitch}
        onClose={() => {
          setShowBackendSwitch(false);
          setBackendSwitchError(null);
        }}
        title={`Switch Local backend to ${nextBackendLabel}?`}
      >
        <div className="flex flex-col gap-3">
          <div className="rounded-lg border border-warning/30 bg-bg-elevated p-3 flex items-start gap-2">
            <AlertTriangle
              size={14}
              className="text-warning flex-shrink-0 mt-0.5"
            />
            <p className="text-xs text-text-secondary leading-relaxed">
              Switching backends does <strong>not</strong> migrate existing
              secrets. Anything you already stored as{" "}
              <strong>{isKeychain ? "Keychain" : "Vault"}</strong> entries will
              stay where they are and won't be readable from the new backend
              until you re-enter them.
              {isKeychain ? (
                <>
                  {" "}
                  If this is your first time enabling the vault, you'll be
                  prompted for a master password on the next screen.
                </>
              ) : (
                <>
                  {" "}
                  Your existing vault file remains on disk. You can switch
                  back later without data loss.
                </>
              )}
            </p>
          </div>
          {backendSwitchError && (
            <p className="text-xs text-negative flex items-center gap-1">
              <AlertTriangle size={12} />
              {backendSwitchError}
            </p>
          )}
          <div className="flex justify-end gap-2 mt-2">
            <PillButton
              variant="outlined"
              onClick={() => {
                setShowBackendSwitch(false);
                setBackendSwitchError(null);
              }}
              disabled={busy}
            >
              Cancel
            </PillButton>
            <PillButton
              variant="brand"
              onClick={confirmBackendSwitch}
              disabled={busy}
              aria-label={`Confirm switch to ${nextBackendLabel}`}
            >
              <Shuffle size={12} className="mr-1" />
              Switch to {nextBackendLabel}
            </PillButton>
          </div>
        </div>
      </Modal>
    </MainContent>
  );
}
