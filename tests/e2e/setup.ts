import type { Page } from "@playwright/test";

/**
 * In-memory state backing the mocked Tauri invoke router. Each test starts
 * with its own fresh `MockState`; mutations from `add_*` / `update_*` /
 * `delete_*` commands are reflected in subsequent `list_*` calls so the
 * React UI can round-trip through state the same way it does in the real
 * Tauri runtime.
 *
 * Shape deliberately mirrors `crates/mcp-proxy-common/src/models.rs` and
 * `src/lib/types.ts` — anything else would drift.
 */
export interface MockServer {
  id: string;
  name: string;
  command: string;
  args: string[];
  transport: { type: "Stdio" } | { type: "Sse"; port: number; path: string };
  env_mappings: Array<{ env_var_name: string; secret_ref: string }>;
  run_mode:
    | { type: "Local" }
    | { type: "DockerSandbox"; image: string | null; extra_args: string[] };
  enabled: boolean;
  trusted: boolean;
  created_at: string;
  updated_at: string;
}

export interface MockSecret {
  id: string;
  label: string;
  source: { type: "Local" } | { type: "OnePassword"; reference: string };
}

export interface MockVault {
  backend: "keychain" | "encrypted-file";
  exists: boolean;
  unlocked: boolean;
}

export interface MockState {
  servers: MockServer[];
  secrets: MockSecret[];
  vault: MockVault;
  autostart: boolean;
  autostartSupported: boolean;
}

export function defaultMockState(overrides: Partial<MockState> = {}): MockState {
  return {
    servers: [],
    secrets: [],
    vault: { backend: "keychain", exists: true, unlocked: true },
    autostart: false,
    autostartSupported: true,
    ...overrides,
  };
}

/**
 * Inject a Tauri v2 IPC mock into the page. Must be called BEFORE
 * `page.goto` — it uses `addInitScript`, which runs before any frame script
 * on every subsequent navigation.
 *
 * Implementation: we serialize the state + a small router into a string and
 * install it as `window.__TAURI_INTERNALS__`. The `invoke` from
 * `@tauri-apps/api/core` calls into that, so every Tauri command the React
 * code fires is routed through our in-memory state.
 */
export async function installTauriMock(
  page: Page,
  state: MockState = defaultMockState(),
): Promise<void> {
  await page.addInitScript((initialState: MockState) => {
    // Deep clone so tests can mutate their local state without affecting
    // the in-page state after navigation, and vice-versa.
    const s: MockState = JSON.parse(JSON.stringify(initialState));

    const nowIso = () => new Date().toISOString();
    const genId = (name: string) =>
      name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "") || `id-${Math.random().toString(36).slice(2, 8)}`;

    const router = async (cmd: string, args: Record<string, unknown> = {}) => {
      switch (cmd) {
        // ---------- Servers ----------
        case "list_servers":
          return s.servers;
        case "get_server": {
          const server = s.servers.find((x) => x.id === args.id);
          if (!server) throw new Error(`server not found: ${args.id}`);
          return server;
        }
        case "add_server": {
          const id = genId(args.name as string);
          const transportType = (args.transportType as string) ?? "stdio";
          const runModeType = (args.runModeType as string | undefined) ?? "local";
          const server: MockServer = {
            id,
            name: args.name as string,
            command: args.command as string,
            args: (args.args as string[]) ?? [],
            transport:
              transportType === "sse"
                ? {
                    type: "Sse",
                    port: (args.ssePort as number) ?? 3000,
                    path: (args.ssePath as string) ?? "/sse",
                  }
                : { type: "Stdio" },
            env_mappings:
              (args.envMappings as Array<{
                env_var_name: string;
                secret_ref: string;
              }>) ?? [],
            run_mode:
              runModeType === "docker"
                ? {
                    type: "DockerSandbox",
                    image: (args.dockerImage as string | null) ?? null,
                    extra_args: [],
                  }
                : { type: "Local" },
            enabled: true,
            trusted: false,
            created_at: nowIso(),
            updated_at: nowIso(),
          };
          s.servers.push(server);
          return server;
        }
        case "update_server": {
          const incoming = args.server as MockServer;
          const idx = s.servers.findIndex((x) => x.id === incoming.id);
          if (idx < 0) throw new Error(`server not found: ${incoming.id}`);
          s.servers[idx] = { ...incoming, updated_at: nowIso() };
          return s.servers[idx];
        }
        case "delete_server": {
          s.servers = s.servers.filter((x) => x.id !== args.id);
          return null;
        }

        // ---------- Secrets ----------
        case "list_secrets":
          return s.secrets;
        case "get_secret":
          return "**redacted-in-mock**";
        case "set_secret": {
          const existing = s.secrets.findIndex((x) => x.id === args.id);
          const entry: MockSecret = {
            id: args.id as string,
            label: args.label as string,
            source: args.source as MockSecret["source"],
          };
          if (existing >= 0) s.secrets[existing] = entry;
          else s.secrets.push(entry);
          return null;
        }
        case "delete_secret": {
          s.secrets = s.secrets.filter((x) => x.id !== args.id);
          return null;
        }

        // ---------- Vault ----------
        case "vault_status":
          return s.vault;
        case "unlock_vault":
          s.vault.unlocked = true;
          return null;
        case "lock_vault":
          s.vault.unlocked = false;
          return null;
        case "change_vault_password":
          return null;
        case "reset_vault":
          s.vault = { backend: "encrypted-file", exists: false, unlocked: false };
          s.secrets = s.secrets.filter((x) => x.source.type !== "Local");
          return null;

        // ---------- Config generation ----------
        case "generate_config":
          return `{\n  "mcpServers": {}\n}`;
        case "get_client_config_info":
          return {
            client: args.client,
            supported: true,
            unsupported_reason: null,
            path: `/tmp/mock-${args.client}.json`,
            exists: false,
          };
        case "write_client_config":
          return {
            path: `/tmp/mock-${args.client}.json`,
            backup_path: null,
            managed_count: s.servers.length,
            preserved_count: 0,
          };

        // ---------- Autostart plugin ----------
        case "plugin:autostart|is_enabled":
          if (!s.autostartSupported) throw new Error("autostart unsupported");
          return s.autostart;
        case "plugin:autostart|enable":
          s.autostart = true;
          return null;
        case "plugin:autostart|disable":
          s.autostart = false;
          return null;

        default:
          throw new Error(`[mock tauri] unknown command: ${cmd}`);
      }
    };

    // Tauri v2 internals shape. `invoke` is what `@tauri-apps/api/core` calls.
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ =
      {
        invoke: (cmd: string, args: Record<string, unknown>) => router(cmd, args),
        transformCallback: (cb: unknown) => cb,
        metadata: { currentWebview: { label: "main" } },
      };
  }, state);
}

/**
 * Convenience fixture: one server, two secrets, vault keychain-ready.
 */
export function populatedState(): MockState {
  return defaultMockState({
    servers: [
      {
        id: "github",
        name: "GitHub",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-github"],
        transport: { type: "Stdio" },
        env_mappings: [{ env_var_name: "GITHUB_TOKEN", secret_ref: "github-pat" }],
        run_mode: { type: "Local" },
        enabled: true,
        trusted: false,
        created_at: "2025-01-01T00:00:00Z",
        updated_at: "2025-01-01T00:00:00Z",
      },
    ],
    secrets: [
      { id: "github-pat", label: "GitHub PAT", source: { type: "Local" } },
      {
        id: "openai-key",
        label: "OpenAI API Key",
        source: { type: "OnePassword", reference: "op://Personal/OpenAI/key" },
      },
    ],
  });
}
