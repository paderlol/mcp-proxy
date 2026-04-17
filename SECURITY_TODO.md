# Security TODO

Known security gaps to address in future iterations.

## High Priority

### 1. Docker Sandbox: Default `--network=none` + whitelist UI
- **Status**: Docker sandbox itself is now implemented (CLI path). `extra_args` is passed through to `docker run` verbatim — users can set `--network=none` manually, but the default is bridge (permissive).
- **Risk**: Malicious MCP server in Docker sandbox can exfiltrate secrets via network by default.
- **Fix**: When `extra_args` doesn't already specify a network mode, default to `--network=none`. Add a Networking row in the UI with radios: None / Bridge / Custom + domain whitelist.
- **Files**: `crates/mcp-proxy-cli/src/docker.rs` (add default), `src/pages/ServerConfig.tsx` (add UI), `models.rs` (maybe add `network_policy` field)

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

### 6. EncryptedFile Backend Implementation
- **Risk**: Currently a TODO stub — users selecting "Local" storage get an error
- **Fix**: Implement AES-256-GCM encryption with Argon2 key derivation from master password
- **Files**: `src-tauri/src/commands/secrets.rs`

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
