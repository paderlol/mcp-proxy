# MCP Proxy

Secret-management desktop app + CLI for MCP (Model Context Protocol) servers.

Store your API keys once in macOS Keychain / encrypted local vault / 1Password, configure MCP servers in a Spotify-inspired dark UI, and the `mcp-proxy` CLI resolves secrets at runtime so they never appear in any AI-client config file.

## Status

Active development. Core loop works end-to-end on macOS:

- Tauri v2 desktop app (Rust + React 19 + TypeScript + Tailwind 4)
- 5 pages: Dashboard, Servers, Secrets, Config, Settings
- 23-entry curated MCP registry (global + China)
- 3 secret backends: Local (Keychain or AES-GCM vault), 1Password via `op`
- `mcp-proxy` CLI with transparent stdio + Docker sandbox modes
- One-click config write to Claude Desktop / Codex / Cursor / Windsurf
- 69 automated tests (55 Rust + 14 Vitest)

See [CLAUDE.md](CLAUDE.md) for architecture details, [TEST_RULES.md](TEST_RULES.md) for the testing policy, and [SECURITY_TODO.md](SECURITY_TODO.md) for known gaps.

## Build

```bash
# Install frontend deps
npm install

# Dev (desktop app with hot reload)
cargo tauri dev

# Production build (DMG)
cargo tauri build

# CLI only
cargo build -p mcp-proxy-cli --release
```

## Test

```bash
cargo test --workspace   # 55 Rust tests
npm test                 # 14 Vitest tests
```

## License

TBD.
