# Security TODO

Known security gaps to address in future iterations.

## High Priority

### 1. Docker Sandbox: Default `--network=none` + whitelist UI ✅ shipped (trust-tiered MVP)
- **Status**: Trust-tiered defaults now applied in [crates/mcp-proxy-cli/src/docker.rs](crates/mcp-proxy-cli/src/docker.rs) (`resolve_network_flag`).
  - `trusted = true` → no injection (Docker's default bridge network).
  - `trusted = false` + no explicit `--network` in `extra_args` → CLI injects `--network=none`, AND the launch gate refuses to run the server at all until the operator either flips the server to Trusted or sets an explicit network flag in `extra_args`.
  - Any user-supplied `--network` / `--net` flag always wins.
- **UI**: [src/pages/ServerConfig.tsx](src/pages/ServerConfig.tsx) shows a "Network Policy" hint card in the Docker Sandbox section describing the effective policy based on current trust state + `extra_args`.
- **Known follow-ups**:
  - No first-class UI form for `extra_args` yet — operators editing it today do so through the JSON store or future custom-policy UI (radios: None / Bridge / Custom + domain whitelist).
  - No typed `network_policy` field in `McpServerConfig`. Parsing from `extra_args` was chosen to avoid a storage migration; revisit if the UI grows a dedicated picker.
- **Risk (residual)**: A trusted server still runs with Docker's default bridge — reviewing trust remains a human responsibility.

### 2. Untrusted Server Warning ✅ shipped
- **Status**: Trust is a first-class, load-bearing flag on every server:
  - **CLI launch gate** ([crates/mcp-proxy-cli/src/main.rs](crates/mcp-proxy-cli/src/main.rs) `run_server`): untrusted Local-mode servers are refused outright; untrusted Docker-sandbox servers only launch if the operator sets an explicit `--network` flag in `extra_args`.
  - **Network policy tiered by trust** (see §1): untrusted sandboxed servers default to `--network=none`; trusted servers keep Docker's default bridge.
  - **UI warnings** ([src/pages/ServerConfig.tsx](src/pages/ServerConfig.tsx)): Trust Level pills with explicit "Launch + Network" copy per state; Network Policy hint card; Local-mode "no isolation" `ShieldAlert` warning.
- **Residual risk**: Flipping a server to Trusted is a one-click action — reviewing trust remains a human responsibility.

### 3. Dockerfile Command Injection (lower priority now)
- **Risk (original)**: Malicious command/args in server config could exploit generated Dockerfile
- **Fix**: Sanitize and shell-escape all user inputs before embedding in Dockerfile RUN statements
- **Files**: Future Dockerfile generation code in `src-tauri/src/commands/proxy.rs`

## Medium Priority

### 4. Audit Log ✅ shipped
- **Status**: Shipped in [crates/mcp-proxy-common/src/audit.rs](crates/mcp-proxy-common/src/audit.rs).
  Every CLI launch (and secret resolution) appends a timestamped record
  keyed by `server_id` + resolved env var names (no secret values) to a
  local JSONL log. The Tauri GUI surfaces the log through
  [src-tauri/src/commands/logs.rs](src-tauri/src/commands/logs.rs) and a
  Settings-page viewer.
- **Residual risk**: Log file is plain JSONL, readable by same-UID
  attackers — acceptable since it contains no secret material, only
  which server accessed which env var names and when.

### 5. Binary Integrity (Code Signing)
- **Risk**: `mcp-proxy` or `mcp-proxy-agent` binary replaced by attacker
- **Fix**: Enable Tauri's macOS code signing in build pipeline. Verify agent binary hash before copying to Docker image.
- **Files**: `src-tauri/tauri.conf.json`, CI/CD config

### 6. EncryptedFile Backend Implementation ✅ shipped (MVP)
- **Status**: Implemented in [crates/mcp-proxy-common/src/vault.rs](crates/mcp-proxy-common/src/vault.rs) —
  AES-256-GCM cipher, 32-byte key derived via Argon2id (19 MiB / 2 iters / 1 lane),
  single-blob JSON plaintext, atomic writes, 12 unit tests.
- **Known residual risks / follow-ups**:
  - ~~`MCP_PROXY_MASTER_PASSWORD` env var leaks via `/proc/PID/environ`~~
    — shipped a session-file fallback:
    [crates/mcp-proxy-common/src/session.rs](crates/mcp-proxy-common/src/session.rs).
    When the GUI unlocks, it writes the derived 32-byte key (not the raw
    password) to `$XDG_RUNTIME_DIR/com.mcp-proxy.app/session.key` (0600 on
    Unix). The CLI tries the session file first and only falls back to
    the env var if absent. The session is deleted on Lock, Change
    Password, Reset, or clean GUI exit.
    Remaining caveats: a same-UID attacker who can read the file still
    gets the key (inherent limit of user-space caching); a hard crash can
    leave the file on disk until next GUI launch (which deletes it on
    startup if stale).
  - ~~No idle auto-lock~~ — shipped. Settings page lets users pick a
    timeout (Never / 5 min / 10 min / 30 min / 1 hr). Any user interaction
    resets the timer; after the timeout the vault is locked automatically.
  - ~~No "change master password" or "reset vault" flows~~ — shipped.
    Settings card has both, guarded by a typed-confirmation modal for
    reset.
  - ~~macOS users keep using Keychain; there is no UI to opt into the vault
    on macOS yet.~~ — shipped. Settings → Security card now exposes a
    "Switch to Local Vault" / "Switch to macOS Keychain" pill on macOS,
    persisted via `preferences::prefer_local_vault` in
    `$data_dir/preferences.json` so the CLI reads the same choice. The
    switch does **not** migrate existing secrets between backends; a
    confirmation modal surfaces that limitation before the flip, and
    Vault → Keychain is blocked while the vault is locked to avoid
    orphaning encrypted data.
- **Files**: `crates/mcp-proxy-common/src/{vault,local_backend}.rs`, `src-tauri/src/commands/vault.rs`, `src/pages/Settings.tsx`

## Low Priority

### 7. Docker stdin Logging
- See §10 — tracked there as the concrete `--log-driver=none` task.

### 8. `/proc/PID/environ` Readable in Container
- **Risk**: Any process in the container can read MCP server's env vars via procfs
- **Fix**: Inherent limitation of env var injection. Mitigated by container isolation (single-process container). Document as known limitation.

### 9. Local Mode No Isolation ✅ shipped on macOS (Linux/Windows pending)
- **Status (macOS)**: Opt-in `sandbox_local` flag on `McpServerConfig`
  wraps Local-mode children in `sandbox-exec(1)` with a generated `.sb`
  profile.
  - Implementation: [crates/mcp-proxy-cli/src/sandbox.rs](crates/mcp-proxy-cli/src/sandbox.rs)
    (profile generation + `TempProfile` RAII guard) wired through
    [crates/mcp-proxy-cli/src/main.rs](crates/mcp-proxy-cli/src/main.rs)
    (`build_local_command_macos`).
  - Profile: `(deny default)` with broad `file-read*` allow + denylist for
    secret stores (`~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gh`,
    `~/Library/Keychains`, `/etc/master.passwd`, `/etc/sudoers`), writes
    scoped to `$TMPDIR` + `~/Library/Caches/mcp-proxy/<id>/`, network
    allowed by default.
  - UI: Local mode in [src/pages/ServerConfig.tsx](src/pages/ServerConfig.tsx)
    exposes a "macOS Sandbox" toggle (hidden on non-macOS via
    `useIsMacos`); the warning copy swaps to the `ShieldCheck`
    "sandbox-exec enabled" variant when on.
  - Fallback: if `sandbox-exec` is missing the CLI logs a warning and
    falls back to direct spawn rather than refusing to launch.
- **Remaining (other platforms)**: No Linux (bubblewrap / Landlock /
  seccomp) or Windows (AppContainer / Job Object) wrapper yet — Local
  mode on those hosts still runs with the user's full FS/network access.
  Trust gate (§2) is the sole defense outside macOS for now.

### 10. Docker `--log-driver=none` by Default ✅ shipped
- **Risk**: Operators who configure non-default Docker log drivers
  (e.g. `journald`, `fluentd`, `splunk`, `gelf`) could capture container
  stdin, which includes the one-line JSON secret payload written by the
  CLI. Docker's default `json-file` driver does not capture stdin, so
  default-configured hosts were safe, but this is defense in depth.
- **Status**: Shipped in
  [crates/mcp-proxy-cli/src/docker.rs](crates/mcp-proxy-cli/src/docker.rs)
  (`resolve_log_driver_flag` + `extra_args_specify_log_driver`, wired
  into `docker_run_with_stdin_payload`). Every `docker run` invocation
  now gets `--log-driver=none` injected by default. Any explicit
  `--log-driver` flag in `extra_args` (single- or space-form) still
  wins. Documented in README.md "Container logging" and README.zh-CN.md
  "容器日志".
- **Residual risk**: Operators who intentionally set `--log-driver` in
  `extra_args` to a capturing driver trade off auditability vs. secret
  exposure — that is an informed choice.

### 11. Base Image Auto-Inference & Prebuilt-Image MCP Servers
- **Risk**: Not strictly a security gap — but both touch the Dockerfile
  that embeds `mcp-proxy-agent`, so listed here for tracking.
  - Today: user must manually specify a base image (e.g. `node:20-alpine`)
    and the CLI always multi-stage-builds agent + MCP server on top.
  - Desired: auto-pick `node:*` for `npx`, `python:*` for `uvx`, etc.;
    and support treating the server *itself* as a prebuilt Docker image
    (run the image directly with agent injected via volume or side-car).
- **Status**: Not yet designed.
