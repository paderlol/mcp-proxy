import { invoke } from "@tauri-apps/api/core";
import type {
  AuditLogEntry,
  ClientConfigInfo,
  EnvMapping,
  McpServerConfig,
  SecretEntry,
  SecretSource,
  VaultStatus,
  WriteConfigResult,
} from "./types";

// Secrets
export const listSecrets = () => invoke<SecretEntry[]>("list_secrets");
export const getSecret = (id: string, source: SecretSource) =>
  invoke<string>("get_secret", { id, source });
/**
 * Upsert a secret. Pass `value = null` to update metadata (label/source)
 * without touching the stored secret value — useful when editing a pre-existing
 * entry and the user leaves the "value" field blank.
 */
export const setSecret = (
  id: string,
  label: string,
  value: string | null,
  source: SecretSource,
) => invoke<void>("set_secret", { id, label, value, source });
export const deleteSecret = (id: string, source: SecretSource) =>
  invoke<void>("delete_secret", { id, source });

// Servers
export const listServers = () => invoke<McpServerConfig[]>("list_servers");
export const getServer = (id: string) =>
  invoke<McpServerConfig>("get_server", { id });
export const addServer = (params: {
  name: string;
  command: string;
  args: string[];
  transportType: string;
  ssePort?: number;
  ssePath?: string;
  runModeType?: string;
  dockerImage?: string;
  envMappings?: EnvMapping[];
  trusted?: boolean;
}) => invoke<McpServerConfig>("add_server", params);
export const updateServer = (server: McpServerConfig) =>
  invoke<McpServerConfig>("update_server", { server });
export const deleteServer = (id: string) =>
  invoke<void>("delete_server", { id });

// Config generation
export const generateConfig = (client: string) =>
  invoke<string>("generate_config", { client });

// Client config write (one-click deploy)
export const getClientConfigInfo = (client: string) =>
  invoke<ClientConfigInfo>("get_client_config_info", { client });
export const writeClientConfig = (client: string) =>
  invoke<WriteConfigResult>("write_client_config", { client });

// Audit logs
export const listAuditLogs = (limit = 50) =>
  invoke<AuditLogEntry[]>("list_audit_logs", { limit });

// Vault / Local-secret lifecycle
export const vaultStatus = () => invoke<VaultStatus>("vault_status");
export const unlockVault = (password: string) =>
  invoke<void>("unlock_vault", { password });
export const lockVault = () => invoke<void>("lock_vault");
export const changeVaultPassword = (newPassword: string) =>
  invoke<void>("change_vault_password", { newPassword });
export const resetVault = () => invoke<void>("reset_vault");
export const setPreferLocalVault = (enabled: boolean) =>
  invoke<void>("set_prefer_local_vault", { enabled });
