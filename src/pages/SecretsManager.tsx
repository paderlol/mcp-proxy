import { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import { MainContent } from "../components/layout/MainContent";
import { Card } from "../components/ui/Card";
import { PillButton } from "../components/ui/PillButton";
import { SecretInput } from "../components/ui/SecretInput";
import { SearchInput } from "../components/ui/SearchInput";
import { Modal } from "../components/ui/Modal";
import { Badge } from "../components/ui/Badge";
import { Plus, KeyRound, Lock, Trash2, Pencil, AlertTriangle } from "lucide-react";
import { useSecrets } from "../hooks/useSecrets";
import { useVault } from "../hooks/useVault";
import type { SecretEntry, SecretSource } from "../lib/types";

type SourceType = "Local" | "OnePassword";

const saveLabels: Record<SourceType, string> = {
  Local: "Save Secret",
  OnePassword: "Save Reference",
};

// Best-effort platform detection for the UI label only.
// Backend decides the actual storage regardless of this string.
function detectLocalBackendLabel(): string {
  const platform =
    typeof navigator !== "undefined" ? navigator.platform || "" : "";
  if (/Mac|iPhone|iPad/.test(platform)) {
    return "macOS Keychain";
  }
  return "AES-256 vault (coming soon)";
}

const sourceBadgeLabel: Record<string, string> = {
  Local: "Local",
  OnePassword: "1Password",
};

export function SecretsManager() {
  const { secrets, fetchSecrets, addSecret, updateSecret, deleteSecret } =
    useSecrets();
  const { status: vaultStatus, refresh: refreshVault } = useVault();

  const [showAdd, setShowAdd] = useState(false);
  // null = creating; non-null = editing this existing entry. The `id` and
  // `source.type` are immutable when editing.
  const [editingSecret, setEditingSecret] = useState<SecretEntry | null>(null);
  const [newId, setNewId] = useState("");
  const [newLabel, setNewLabel] = useState("");
  const [newValue, setNewValue] = useState("");
  const [sourceType, setSourceType] = useState<SourceType>("Local");
  const [opReference, setOpReference] = useState("");
  const [saving, setSaving] = useState(false);
  const [query, setQuery] = useState("");

  useEffect(() => {
    fetchSecrets();
    refreshVault();
  }, [fetchSecrets, refreshVault]);

  // On macOS the vault is always "unlocked" (Keychain). On Linux/Windows, if
  // the user tries to save a Local secret while the vault is locked we show
  // an inline banner pointing to Settings instead of the value input.
  const vaultBlocksLocal =
    sourceType === "Local" &&
    vaultStatus !== null &&
    vaultStatus.backend === "encrypted-file" &&
    !vaultStatus.unlocked;

  const resetForm = () => {
    setNewId("");
    setNewLabel("");
    setNewValue("");
    setSourceType("Local");
    setOpReference("");
    setEditingSecret(null);
    setShowAdd(false);
  };

  const handleEdit = (secret: SecretEntry) => {
    setEditingSecret(secret);
    setNewId(secret.id);
    setNewLabel(secret.label);
    setNewValue("");
    if (secret.source.type === "OnePassword") {
      setSourceType("OnePassword");
      setOpReference(secret.source.reference);
    } else {
      setSourceType("Local");
      setOpReference("");
    }
    setShowAdd(true);
  };

  const handleSave = async () => {
    if (!newId || !newLabel) return;
    setSaving(true);
    try {
      const source: SecretSource =
        sourceType === "OnePassword"
          ? { type: "OnePassword", reference: opReference }
          : { type: "Local" };

      if (editingSecret) {
        // On edit, an empty "value" field means "keep the existing Keychain
        // entry unchanged" (only relevant for Local secrets). For 1Password
        // we pass null since there's no stored value anyway.
        const value: string | null =
          sourceType === "OnePassword"
            ? null
            : newValue.length > 0
              ? newValue
              : null;
        await updateSecret(newId, newLabel, value, source);
      } else {
        const value = sourceType === "OnePassword" ? "" : newValue;
        await addSecret(newId, newLabel, value, source);
      }
      resetForm();
    } finally {
      setSaving(false);
    }
  };

  const inputClass =
    "w-full bg-bg-elevated text-text-primary rounded-[500px] px-4 py-2.5 text-sm outline-none border border-transparent focus:border-border-default shadow-[rgb(18,18,18)_0px_1px_0px,rgb(124,124,124)_0px_0px_0px_1px_inset] placeholder:text-text-secondary/50";

  const filteredSecrets = query.trim()
    ? secrets.filter((s) => {
        const haystack = [
          s.id,
          s.label,
          s.source.type,
          s.source.type === "OnePassword" ? s.source.reference : "",
        ]
          .join(" ")
          .toLowerCase();
        return haystack.includes(query.toLowerCase().trim());
      })
    : secrets;

  return (
    <MainContent
      title="Secrets"
      description="Store secrets locally on this device, or reference them from 1Password"
      actions={
        <PillButton variant="brand" onClick={() => setShowAdd(true)}>
          <Plus size={14} className="mr-1.5" />
          Add Secret
        </PillButton>
      }
    >
      <div className="mb-6">
        <SearchInput
          placeholder="Search by id, label, source…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {secrets.length === 0 ? (
        <Card>
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <KeyRound size={40} className="text-text-secondary/40 mb-3" />
            <p className="text-sm text-text-secondary mb-1">
              No secrets stored yet
            </p>
            <p className="text-xs text-text-secondary/60">
              Click "Add Secret" to store your first API key
            </p>
          </div>
        </Card>
      ) : filteredSecrets.length === 0 ? (
        <Card>
          <div className="flex flex-col items-center justify-center py-10 text-center">
            <p className="text-sm text-text-secondary">
              No secrets match "{query}"
            </p>
          </div>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {filteredSecrets.map((s) => (
            <Card key={s.id}>
              <div className="flex items-center justify-between">
                <div>
                  <div className="flex items-center gap-2 mb-0.5">
                    <p className="text-sm font-bold text-text-primary">
                      {s.label}
                    </p>
                    <Badge>
                      {sourceBadgeLabel[s.source.type] ?? s.source.type}
                    </Badge>
                  </div>
                  <p className="text-xs text-text-secondary font-mono">
                    {s.id}
                    {s.source.type === "OnePassword" &&
                      "reference" in s.source &&
                      ` — ${s.source.reference}`}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <PillButton
                    variant="outlined"
                    onClick={() => handleEdit(s)}
                    aria-label={`Edit ${s.label}`}
                  >
                    <Pencil size={12} />
                  </PillButton>
                  <PillButton
                    variant="outlined"
                    onClick={() => deleteSecret(s.id, s.source)}
                    className="!text-negative hover:!border-negative"
                    aria-label={`Delete ${s.label}`}
                  >
                    <Trash2 size={12} />
                  </PillButton>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      <Modal
        open={showAdd}
        onClose={resetForm}
        title={editingSecret ? `Edit Secret: ${editingSecret.label}` : "Add Secret"}
      >
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Source {editingSecret && <span className="text-text-secondary/60 font-normal">(can't change on edit)</span>}
            </label>
            <div className="flex gap-2">
              <PillButton
                variant={sourceType === "Local" ? "dark" : "outlined"}
                onClick={() => !editingSecret && setSourceType("Local")}
                disabled={!!editingSecret}
                className="flex-1"
              >
                <Lock size={14} className="mr-1.5" />
                Local
              </PillButton>
              <PillButton
                variant={sourceType === "OnePassword" ? "dark" : "outlined"}
                onClick={() => !editingSecret && setSourceType("OnePassword")}
                disabled={!!editingSecret}
                className="flex-1"
              >
                <KeyRound size={14} className="mr-1.5" />
                1Password
              </PillButton>
            </div>
            <p className="text-xs text-text-secondary/60 px-1">
              {sourceType === "Local" && (
                <>
                  Stored securely on this device ({detectLocalBackendLabel()}).
                </>
              )}
              {sourceType === "OnePassword" &&
                "Fetched live via `op read` each time the proxy runs. Requires 1Password CLI."}
            </p>
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Secret ID {editingSecret && <span className="text-text-secondary/60 font-normal">(immutable)</span>}
            </label>
            <input
              type="text"
              value={newId}
              onChange={(e) => setNewId(e.target.value)}
              placeholder="e.g., github-pat"
              disabled={!!editingSecret}
              className={`${inputClass} ${editingSecret ? "opacity-60 cursor-not-allowed" : ""}`}
            />
            {editingSecret && (
              <p className="text-xs text-text-secondary/60 px-1">
                The ID is referenced by server env mappings — renaming would
                break them. Delete and re-add if you need a different ID.
              </p>
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <label className="text-sm text-text-secondary font-bold">
              Label
            </label>
            <input
              type="text"
              value={newLabel}
              onChange={(e) => setNewLabel(e.target.value)}
              placeholder="e.g., GitHub Personal Access Token"
              className={inputClass}
            />
          </div>

          {sourceType === "OnePassword" ? (
            <div className="flex flex-col gap-1.5">
              <label className="text-sm text-text-secondary font-bold">
                1Password Reference
              </label>
              <input
                type="text"
                value={opReference}
                onChange={(e) => setOpReference(e.target.value)}
                placeholder="op://vault/item/field"
                className={`${inputClass} font-mono`}
              />
            </div>
          ) : vaultBlocksLocal ? (
            <div className="rounded-lg border border-warning/30 bg-bg-elevated p-3 flex items-start gap-2">
              <AlertTriangle
                size={14}
                className="text-warning flex-shrink-0 mt-0.5"
              />
              <div className="text-xs text-text-secondary leading-relaxed">
                <p className="text-text-primary font-bold mb-1">
                  Local vault is locked
                </p>
                <p>
                  Unlock or create the vault in{" "}
                  <Link
                    to="/settings"
                    className="text-brand underline hover:brightness-110"
                  >
                    Settings
                  </Link>{" "}
                  before saving a Local secret. On Linux and Windows, Local
                  secrets are protected by a master password you set once.
                </p>
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-1.5">
              <SecretInput
                label={
                  editingSecret ? "New Secret Value (optional)" : "Secret Value"
                }
                value={newValue}
                onChange={(e) => setNewValue(e.target.value)}
                placeholder={
                  editingSecret
                    ? "Leave blank to keep current value"
                    : "Paste your API key or token"
                }
              />
              {editingSecret && (
                <p className="text-xs text-text-secondary/60 px-1">
                  Leave empty to keep the stored value; type a new value to
                  replace it.
                </p>
              )}
            </div>
          )}

          <div className="flex justify-end gap-2 mt-2">
            <PillButton variant="outlined" onClick={resetForm}>
              Cancel
            </PillButton>
            <PillButton
              variant="brand"
              onClick={handleSave}
              disabled={!newId || !newLabel || saving || vaultBlocksLocal}
            >
              {saving
                ? "Saving..."
                : editingSecret
                  ? "Update"
                  : saveLabels[sourceType]}
            </PillButton>
          </div>
        </div>
      </Modal>
    </MainContent>
  );
}
