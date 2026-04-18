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

### 2. Untrusted Server Warning
- **Risk**: Users may add malicious MCP servers without realizing the risk
- **Fix**: Show confirmation dialog before first launch of any server with `trusted: false`
- **UI**: "This server is not verified. It will have access to: [list of secrets]. Continue?"
- **Files**: `src/pages/ServerConfig.tsx`, `src-tauri/src/commands/proxy.rs`

### 3. Dockerfile Command Injection (lower priority now)
- **Risk (original)**: Malicious command/args in server config could exploit generated Dockerfile
- **Fix**: Sanitize and shell-escape all user inputs before embedding in Dockerfile RUN statements
- **Files**: Future Dockerfile generation code in `src-tauri/src/commands/proxy.rs`

## Medium Priority

### 4. Audit Log
- **Risk**: No visibility into which server accessed which secrets and when
- **Fix**: Log all `resolve_secret()` calls with timestamp, server_id, secret_id to a local log file
- **UI**: Add log viewer to Settings page
- **Files**: `src-tauri/src/commands/proxy.rs`, `src/pages/Settings.tsx`

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
- **Risk**: Custom Docker logging drivers may capture stdin, exposing the secret JSON payload
- **Fix**: Document this in user-facing README. Consider adding a `--log-driver=none` flag to docker run.

### 8. `/proc/PID/environ` Readable in Container
- **Risk**: Any process in the container can read MCP server's env vars via procfs
- **Fix**: Inherent limitation of env var injection. Mitigated by container isolation (single-process container). Document as known limitation.

### 9. Local Mode No Isolation
- **Risk**: Local mode MCP server has full user filesystem/network access
- **Fix**: Document risk clearly in UI. Consider optional macOS `sandbox-exec` integration in future.
