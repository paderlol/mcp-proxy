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
  created_at: string;
  updated_at: string;
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

export interface ProxyStatus {
  server_id: string;
  running: boolean;
  pid: number | null;
}

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
  /** "keychain" on macOS, "encrypted-file" otherwise */
  backend: "keychain" | "encrypted-file";
  exists: boolean;
  unlocked: boolean;
}
