# Contributing to MCP Proxy

Thanks for your interest. Issues and pull requests are welcome.

## Quick orientation

- **Architecture**: see [CLAUDE.md](CLAUDE.md) — data model, crate boundaries, run modes, the three secret backends, and the Docker sandbox design.
- **UI system**: see [docs/DESIGN.md](docs/DESIGN.md) — Spotify-inspired dark theme with specific typography, spacing, and component rules. Follow this for any new UI work.
- **Testing policy**: see [docs/TEST_RULES.md](docs/TEST_RULES.md) — §3 lists tests that are **mandatory** for specific kinds of changes. Read it before opening a PR that touches data models, secret handling, the CLI, or file-writing commands.
- **Known security gaps**: see [docs/SECURITY_TODO.md](docs/SECURITY_TODO.md) — if your change touches any item listed there, mention it in the PR description.

## Prerequisites

- Rust stable (1.80+)
- Node.js 20+
- On macOS: Xcode Command Line Tools (for Tauri)

```bash
git clone git@github.com:paderlol/mcp-proxy.git
cd mcp-proxy
npm install
```

## Local development

```bash
# Desktop app (hot reload for frontend, rebuild for Rust)
cargo tauri dev

# Frontend only (http://localhost:1420, Tauri commands will fail)
npm run dev

# CLI
cargo build -p mcp-proxy-cli --release
./target/release/mcp-proxy --help
```

## Running tests

```bash
# Full suite
cargo test --workspace    # 55 Rust tests
npm test                  # 14 Vitest tests

# Single crate while iterating
cargo test -p mcp-proxy-common
cargo test -p mcp-proxy-cli
cargo test -p mcp-proxy

# Docker sandbox integration (requires Docker; slow)
cargo test -p mcp-proxy-cli -- --ignored docker
```

## Pull request checklist

Before submitting:

- [ ] `cargo test --workspace` passes
- [ ] `npm test` passes
- [ ] `npm run build` passes (TypeScript strict + Vite)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace -- -D warnings` has no new warnings
- [ ] New code paths have tests per [TEST_RULES.md §3](docs/TEST_RULES.md)
- [ ] No secrets or personal paths in committed files

CI runs the same checks on every push.

## Commit messages

- First line: imperative, ≤ 70 chars (e.g., "Add AES-GCM vault backend")
- Body: explain *why*, not just what — link issues, mention tradeoffs
- Reference any `docs/TEST_RULES.md` / `docs/SECURITY_TODO.md` item the PR touches

## Project scope

MCP Proxy is a **secret-management layer** for MCP servers — keep changes aligned with that. If you have ideas for features outside that scope (e.g., building an MCP server of our own), open an issue first to discuss.

## Security

**Never commit a real secret, API key, or personal token.** The `.gitignore` catches common filenames but is not a replacement for care.

To report a security vulnerability privately, open a GitHub Security Advisory on this repo rather than a public issue.
