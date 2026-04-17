# Manual End-to-End Verification with AI Clients

The automated test suite ([crates/mcp-proxy-cli/tests/e2e_stdio_pipe.rs](../crates/mcp-proxy-cli/tests/e2e_stdio_pipe.rs)) proves stdio is transparently piped through the CLI using `/bin/cat` as a stand-in server. That covers the protocol transport.

**This checklist is for the parts only a human can verify**: that a real AI client (Claude Desktop, Codex, Cursor, VS Code, Windsurf) can talk to a real MCP server through `mcp-proxy run`, end-to-end, on a real machine.

Run through this checklist whenever you cut a release, or whenever the CLI or config-generation code changes in a non-trivial way.

---

## Prerequisites

- macOS (Linux/Windows still need broader end-to-end validation; see SECURITY_TODO §6)
- `cargo tauri build` succeeded → `./target/release/mcp-proxy` exists and is executable
- The AI client you're testing is installed

---

## One-time setup

### 1. Put `mcp-proxy` on `$PATH`

The generated config files reference `mcp-proxy` by name, so it needs to be discoverable.

```bash
# Option A: symlink from /usr/local/bin (simplest)
sudo ln -sf "$(pwd)/target/release/mcp-proxy" /usr/local/bin/mcp-proxy

# Option B: copy to ~/.local/bin if it's already on your PATH
mkdir -p ~/.local/bin
cp target/release/mcp-proxy ~/.local/bin/

# Verify
which mcp-proxy
mcp-proxy --version   # should print "mcp-proxy 0.1.0"
```

### 2. Launch the desktop app

```bash
cargo tauri dev
```

### 3. Add a test MCP server through the UI

Use a secretless server for the smoke test — `filesystem` is ideal because it doesn't need any API keys:

1. Go to the **Servers** page → click **Browse** → select **Filesystem** (under Global)
2. Click **Install** → update the path argument to a real directory you want it to read, e.g. `/tmp/mcp-test`
3. `mkdir /tmp/mcp-test && echo hello > /tmp/mcp-test/greeting.txt`
4. Click **Save Server**

Confirm it's persisted:

```bash
mcp-proxy list
# should show the filesystem server with command npx, args -y @modelcontextprotocol/server-filesystem /tmp/mcp-test
```

---

## Per-client verification

Pick the client(s) you care about. Each one reads a different config file.

### Claude Desktop

1. In the desktop app: **Config** page → select **Claude Desktop** → **Copy**
2. Open `~/Library/Application Support/Claude/claude_desktop_config.json` in an editor
3. Paste (merge with existing `mcpServers` if any)
4. Fully quit Claude Desktop (⌘Q, not just close window) and reopen
5. In a new conversation, ask Claude: *"What tools do you have available?"*
6. ✅ **Pass criterion**: Claude lists tools prefixed with `filesystem.` (e.g. `filesystem.read_file`, `filesystem.list_directory`)
7. Ask Claude: *"Read /tmp/mcp-test/greeting.txt"*
8. ✅ **Pass criterion**: Claude calls `filesystem.read_file` and returns `hello`

### Codex CLI

1. **Config** page → **Codex** → **Copy**
2. Paste into `~/.codex/config.toml` (merge with existing `[mcp_servers.*]` if any)
3. Restart the Codex CLI
4. Run a task that requires filesystem access
5. ✅ **Pass criterion**: Codex's tool calls hit the filesystem server and return real file contents

### Cursor

1. **Config** page → **Cursor** → **Copy**
2. Paste into `~/.cursor/mcp.json`
3. Cursor picks up changes hot — no restart needed (hit 🔄 in the MCP settings panel)
4. Open the MCP panel: Settings → MCP
5. ✅ **Pass criterion**: the filesystem server shows with a green status dot

### VS Code (with MCP extension)

1. **Config** page → **VS Code** → **Copy**
2. Paste into `.vscode/mcp.json` in the project you want the server scoped to
3. Reload the VS Code window
4. ✅ **Pass criterion**: the filesystem server appears in the MCP extension's server list

### Windsurf

1. **Config** page → **Windsurf** → **Copy**
2. Paste into `~/.codeium/windsurf/mcp_config.json`
3. Restart Windsurf
4. ✅ **Pass criterion**: filesystem tools are callable from the AI chat panel

---

## Verifying secret injection (with a real API key)

The filesystem test above proves the transport works but doesn't exercise secret resolution. For a real end-to-end secrets test, use something with a free tier:

1. Get a **Brave Search API key** (free tier, takes 2 minutes): <https://brave.com/search/api/>
2. In the desktop app: **Secrets** page → **Add Secret** → `Local` backend
   - Secret ID: `brave-api-key`
   - Label: `Brave Search API Key`
   - Value: paste your key
3. **Servers** page → **Browse** → select **Brave Search** → **Install**
4. In the prefilled form, under **Environment Variables**: the row `BRAVE_API_KEY → ?` should appear. Pick `brave-api-key` from the dropdown.
5. Save the server
6. Regenerate and copy the Claude Desktop config, restart Claude
7. In Claude: *"Search the web for 'anthropic model context protocol'"*
8. ✅ **Pass criteria**:
   - Claude calls `brave.web_search`
   - Real search results come back (proving the API key was injected into the child process)
   - `security find-generic-password -s com.mcp-proxy -a brave-api-key -w` still returns your key (not corrupted)

---

## Negative tests (quick sanity checks)

Worth running once per release — proves errors surface cleanly instead of hanging:

1. **Delete a secret that's still referenced**
   - Remove `brave-api-key` in the Secrets page
   - Call the Brave search tool from Claude
   - ✅ Expected: Claude reports the tool call failed with `Secret 'brave-api-key' not found`

2. **Disable a server**
   - Set `enabled: false` on the filesystem server via the UI (if/when that toggle exists; otherwise skip)
   - Call a filesystem tool
   - ✅ Expected: error mentioning "disabled"

3. **Rename `mcp-proxy` binary**
   - `sudo mv /usr/local/bin/mcp-proxy /usr/local/bin/mcp-proxy.bak`
   - Restart Claude
   - ✅ Expected: Claude reports `command not found: mcp-proxy` (good — fails fast and legibly)
   - Restore: `sudo mv /usr/local/bin/mcp-proxy.bak /usr/local/bin/mcp-proxy`

---

## What this checklist does *not* cover

These still need a different kind of test or a different tool:

- **Concurrent proxies** (two servers running at once) — worth spot-checking manually; not in this checklist yet
- **Long-running sessions** (what happens after 1h of MCP traffic) — no test for this yet
- **Docker sandbox mode** — implemented in the CLI/runtime path, but this checklist has not yet been expanded with a dedicated sandbox validation section
- **Linux / Windows `Local` backend** — implemented as an encrypted local vault path, but still needs a separate cross-platform checklist

---

## Reporting failures

If any step above fails, capture:

1. `mcp-proxy list` output
2. `security find-generic-password -s com.mcp-proxy` list (macOS)
3. The exact config file you pasted
4. The AI client's version and OS
5. Any error shown in the client

File as a GitHub issue with label `bug:e2e`.
