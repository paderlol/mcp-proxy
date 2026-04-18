export interface McpServerConfig {
  id: string;
  name: string;
  command: string;
  args: string[];
  transport: Transport;
  env_mappings: EnvMapping[];
  run_mode: RunMode;
  enabled: boolean;
  trusted: boolean;
  /**
   * macOS-only: wrap Local-mode child processes with `sandbox-exec` and a
   * generated `.sb` profile. Ignored by the CLI on non-macOS hosts.
   */
  sandbox_local?: boolean;
  created_at: string;
  updated_at: string;
  first_launched_at?: string;
}

export type RunMode =
  | { type: "Local" }
  | { type: "DockerSandbox"; image: string | null; extra_args: string[] };

export type Transport =
  | { type: "Stdio" }
  | { type: "Sse"; port: number; path: string };

export interface EnvMapping {
  env_var_name: string;
  secret_ref: string;
}

export interface SecretEntry {
  id: string;
  label: string;
  source: SecretSource;
}

export type SecretSource =
  | { type: "Local" }
  | { type: "OnePassword"; reference: string };

export interface ClientConfigInfo {
  client: string;
  supported: boolean;
  unsupported_reason: string | null;
  path: string | null;
  exists: boolean;
}

export interface WriteConfigResult {
  path: string;
  backup_path: string | null;
  managed_count: number;
  preserved_count: number;
}

export interface VaultStatus {
  /**
   * "keychain" on macOS by default, "encrypted-file" everywhere else and on
   * macOS when the user has opted into the local vault.
   */
  backend: "keychain" | "encrypted-file";
  exists: boolean;
  unlocked: boolean;
  /**
   * macOS opt-in flag mirroring the persisted preference. On non-macOS this
   * is always `false`.
   */
  prefer_local_vault: boolean;
  /** `true` only on macOS — the only platform with a meaningful choice. */
  can_switch_backend: boolean;
}

export type AuditLogStatus =
  | { type: "Success" }
  | { type: "Error"; message: string };

export interface AuditLogEntry {
  timestamp: string;
  server_id: string;
  secret_id: string;
  source: string;
  status: AuditLogStatus;
}
