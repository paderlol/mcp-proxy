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
  log_invocations?: boolean;
}

export type RunMode =
  | { type: "Local" }
  | { type: "DockerSandbox"; image: string | null; extra_args: string[] };

export type Transport =
  | { type: "Stdio" }
  | { type: "Sse"; port: number; path: string };

export type EnvValue =
  | { type: "Secret"; secret_ref: string }
  | { type: "Plaintext"; value: string };

export interface EnvMapping {
  env_var_name: string;
  value?: EnvValue;
  /** Legacy mirror field for downgraded binaries. Set iff `value.type === "Secret"`. */
  secret_ref?: string;
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

export interface InvocationSession {
  id: number;
  server_id: string;
  run_mode: string;
  started_at: string;
  ended_at: string | null;
  exit_code: number | null;
  error: string | null;
  tool_call_count: number;
}

export interface ToolCallRow {
  id: number;
  session_id: number;
  direction: "request" | "response" | "notification" | string;
  method: string | null;
  tool_name: string | null;
  jsonrpc_id: string | null;
  timestamp: string;
  duration_ms: number | null;
  is_error: boolean;
  payload: string;
}

export type SourceClient =
  | "ClaudeDesktop"
  | "ClaudeCode"
  | "Codex"
  | "Cursor"
  | "VsCode"
  | "Windsurf";

export interface DiscoveredServer {
  source: SourceClient;
  source_path: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  transport: string;
}

export type EnvDecision =
  | {
      kind: "Secret";
      env_var_name: string;
      secret_id: string;
      label: string;
      value: string;
    }
  | { kind: "Plaintext"; env_var_name: string; value: string };

export interface ImportSelection {
  discovered: DiscoveredServer;
  env_decisions: EnvDecision[];
  trusted: boolean;
}

export interface ImportResult {
  server_id: string;
  server_name: string;
  created_secret_ids: string[];
}
