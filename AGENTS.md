# MCP Proxy

A secret management proxy for MCP servers — provides secure secret storage, transparent proxying, and optional Docker sandboxing.

## Workflow Rules

Before making code changes, read [rules.md](rules.md).

- `rules.md` is the source of truth for branch selection, fork target, branch naming, commits, and PR workflow.
- Repository-local rules in `rules.md` override generic agent defaults, including any default branch prefix from external tooling.
- Default flow: branch from `main`, keep one branch per task, and do not commit directly to `main`.
- If write access to `paderlol/mcp-proxy` is unavailable, fork `paderlol/mcp-proxy` and open the PR back to upstream `main`.

## Overview

Solves the problem of MCP servers lacking a unified secret management solution. Users manage API keys/tokens through a desktop app, which automatically injects secrets as environment variables into MCP server processes.

**Two run modes:**

```
Mode 1: Local (default, fast)
  AI Client → mcp-proxy run <server-id> → resolve secrets → spawn process → pipe stdio

Mode 2: Docker Sandbox (optional, isolated)
  AI Client → mcp-proxy run <server-id> --sandbox
    → start ephemeral localhost API with one-time token
    → docker run -e TOKEN=xxx <image>
    → container fetches secrets via token → runs MCP server
    → token expires, API shuts down
```

**Multi-client config generation**: Generates MCP config for Codex Desktop, Codex (TOML), Cursor, VS Code, Windsurf.

## Tech Stack

- **Backend**: Rust + Tauri v2
- **Frontend**: React 19 + TypeScript + Vite 6 + Tailwind CSS 4
- **Secret Storage**: Encrypted local file (AES-256-GCM) / macOS Keychain / 1Password CLI (`op`)
- **State Management**: Zustand
- **Routing**: React Router 7
- **Icons**: Lucide React

## Commands

```bash
# Development
npm install                     # Install frontend dependencies
cargo tauri dev                 # Start dev mode (frontend + Rust backend)
npm run dev                     # Frontend only (localhost:1420)

# Build
cargo tauri build               # Production build (DMG)
npm run build                   # Frontend only build


# Test — Rust (78 tests)
cargo test --workspace
cargo test -p mcp-proxy-common   # 9 unit + 14 vault + 6 session = 29 tests
cargo test -p mcp-proxy-cli      # 13 docker unit + 10 CLI integ + 2 stdio E2E
cargo test -p mcp-proxy          # 6 config + 18 client_write = 24 tests

# Test — Frontend (14 Vitest tests)
npm test
npm run test:watch               # watch mode

# Test — Frontend E2E (Playwright, mocked Tauri invoke)
npx playwright install --with-deps chromium   # one-time
npm run test:e2e                 # headless run
npm run test:e2e:ui              # interactive debugger

# Data-dir override for tests & isolated profiles
MCP_PROXY_DATA_DIR=/tmp/mcp-proxy-profile ./target/release/mcp-proxy list
```

See [docs/TEST_RULES.md](docs/TEST_RULES.md) for the full testing policy and
[docs/e2e-manual.md](docs/e2e-manual.md) for manual AI-client verification.

## Project Structure

```
mcp-proxy/
├── src-tauri/                  # Rust backend (Tauri v2)
│   └── src/
│       ├── commands/           # Tauri IPC commands (secrets, servers, proxy)
│       ├── secrets/            # Keychain + 1Password + EncryptedFile backends
│       ├── config/             # MCP server config management
│       └── proxy/              # stdio/SSE transparent proxy + Docker sandbox
├── src/                        # React frontend
│   ├── components/
│   │   ├── layout/             # Sidebar, MainContent
│   │   └── ui/                 # PillButton, Card, Modal, SecretInput, etc.
│   ├── pages/                  # Dashboard, Servers, Secrets, Config, Settings
│   ├── hooks/                  # Custom hooks
│   └── lib/                    # Tauri invoke wrappers, type definitions
├── crates/
│   └── mcp-proxy-common/       # Shared data types (models)
└── docs/DESIGN.md              # UI design system (Spotify-inspired dark theme)
```

## Cargo Workspace

Three crates:
- `src-tauri`: Main app (Tauri desktop)
- `crates/mcp-proxy-common`: Shared data types
- `crates/mcp-proxy-agent`: Tiny binary for inside Docker sandbox containers

## Design System

See `docs/DESIGN.md`. Key points:
- **Dark theme**: backgrounds `#121212` / `#181818` / `#1f1f1f`
- **Brand color**: `#1ed760` (functional highlights only — CTAs, active states)
- **Buttons**: Pill shape (radius 500px–9999px), uppercase + wide letter-spacing
- **Cards**: `#181818` background, 8px border-radius
- **Shadows**: Heavy shadows (0.3–0.5 opacity)

## Coding Conventions

### Rust
- Tauri commands in `src-tauri/src/commands/`, business logic in domain modules
- All Tauri commands return `Result<T, String>` for frontend error handling
- Secrets are NEVER logged, NEVER stored in plaintext
- Use `zeroize` crate to clear secrets from memory after use

### Frontend
- Page components in `src/pages/`
- Reusable UI components in `src/components/ui/`
- Layout components in `src/components/layout/`
- Tauri invocations wrapped in `src/lib/tauri.ts` (typed)
- Type definitions in `src/lib/types.ts`

### Data Models

Core types in `crates/mcp-proxy-common/src/models.rs`:
- `McpServerConfig`: MCP server config (name, command, args, transport, env mappings, run mode, trusted)
- `EnvMapping`: env var name → secret reference mapping
- `SecretMeta`: Secret metadata (actual value lives in storage backend only)
- `SecretSource`: Keychain | OnePassword | EncryptedFile
- `RunMode`: Local | DockerSandbox (image, extra_args)
- `Transport`: Stdio | Sse

## Secret Sources

Three backends:

### Encrypted Local File (Linux / Windows default)
- AES-256-GCM ciphertext with per-write nonce, stored at `$data_dir/vault.bin`
- 32-byte key derived via Argon2id (OWASP "interactive" params) from user master password
- Key held in-memory as `Zeroizing<[u8; 32]>` while the process runs
- Tauri GUI: **Settings → Local Vault** card to unlock / create / lock
- CLI: tries a GUI-written session file first (`$XDG_RUNTIME_DIR/com.mcp-proxy.app/session.key`, 0600), falls back to `MCP_PROXY_MASTER_PASSWORD` env var
- See [crates/mcp-proxy-common/src/vault.rs](crates/mcp-proxy-common/src/vault.rs) for the file format and threat model

### macOS Keychain (macOS default — preferred when available)
- Stored via `keyring` crate with service name `com.mcp-proxy`
- Hardware-backed encryption on Apple Silicon

### 1Password (via `op` CLI)
- Reads secrets via `op read "op://vault/item/field"` at proxy start time
- Never cached locally — fetched fresh each time
- Requires `op` CLI: `brew install --cask 1password-cli`

## Run Modes

### Local (default)
- Spawns MCP server as a local child process with secrets injected as env vars
- Fast, zero overhead — transparent stdio pipe
- No isolation: process has full filesystem/network access

### Docker Sandbox (optional, MVP implemented)
- Runs MCP server in a Docker container for filesystem + network isolation
- Ideal for untrusted / third-party MCP servers
- **Implemented in CLI** ([crates/mcp-proxy-cli/src/docker.rs](crates/mcp-proxy-cli/src/docker.rs)): auto-Dockerfile generation, content-hashed image cache, stdin secret injection via `mcp-proxy-agent`
- **No GUI start/launch path currently exists** — the desktop app configures Docker mode, but actual execution happens later when an AI client invokes `mcp-proxy run <server-id>`
- **First build ~2 min** (Rust compiles the agent in a multi-stage `rust:alpine` build stage). Cached afterwards.
- **User must specify a base image** (e.g., `node:20-alpine` for npx-based servers). No auto-detection yet.
- Network policy: MVP leaves bridge network on; pass `--network=none` via `extra_args` to lock down. See SECURITY_TODO §1.

**Auto-build flow** (transparent to user):
1. Desktop app generates a Dockerfile per server config:
   - Base image matching the MCP server's runtime (node, python, etc.)
   - Installs the MCP server package
   - Copies `mcp-proxy-agent` binary (statically linked Rust, ~3MB)
   - ENTRYPOINT is `mcp-proxy-agent`
2. `docker build` runs locally — no remote image registry needed
3. At runtime (`mcp-proxy run <id> --sandbox`):
   - Resolves secrets from backend (Keychain / 1Password / EncryptedFile)
   - `docker run -i --rm <local-image>`
   - Writes secrets JSON as **first line to docker stdin**
   - `mcp-proxy-agent` reads first line → exec's MCP server with env vars
   - All subsequent stdin/stdout is pure MCP protocol traffic
- No HTTP server, no tokens, no env vars — secrets pass through stdin only
- Secrets never appear in Dockerfile, image layers, `docker inspect`, or process listings

**Stdin protocol**: First line written to container stdin:
```json
{"env_vars": {"GITHUB_TOKEN": "ghp_xxx"}, "command": "npx", "args": ["-y", "@mcp/server-github"]}
```

**`mcp-proxy-agent` crate** (`crates/mcp-proxy-agent/`):
- Tiny Rust binary (~1MB) for use inside Docker sandbox containers
- Reads first line from stdin as JSON secret payload
- Injects env vars, then unix `exec()` replaces itself with the real MCP server
- After exec, stdin/stdout are inherited — MCP traffic flows transparently

## Supported AI Clients (Config Generation)

| Client | Config File | Format |
|--------|-------------|--------|
| Codex Desktop | `~/Library/Application Support/Codex/claude_desktop_config.json` | JSON (`mcpServers` object) |
| Codex | `~/.codex/config.toml` | TOML (`[mcp_servers.id]` tables) |
| Cursor | `~/.cursor/mcp.json` | JSON (`mcpServers` object) |
| VS Code | `.vscode/mcp.json` | JSON (`servers` object, `type: "stdio"`) |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | JSON (`servers` object) |

## Security Model

### Secret Protection
- Secret values stored only in Keychain, 1Password, or AES-256-GCM encrypted vault
- Secrets zeroed from memory after use via `zeroize`
- Secrets never appear in any config file — resolved at runtime only
- Encrypted vault uses Argon2 key derivation to resist brute-force
- Secret scoping: each server only receives its own mapped env vars

### MCP Server Trust
- Each server has a `trusted` flag (default: false)
- Untrusted servers show a warning before first launch
- Audit log records which server accessed which secrets and when

### Docker Sandbox (for untrusted servers)
- Full filesystem isolation — container cannot read host files
- Network can be restricted via Docker `--network` flags
- Secrets delivered via stdin pipe — never visible in env vars, process list, or docker inspect
- No HTTP server, no tokens, no network-based secret delivery
