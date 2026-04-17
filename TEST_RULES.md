# Test Rules

Testing policy for the MCP Proxy project — what's been verified, what's required for new changes, and how to run tests.

---

## 1. Current State

**84 automated tests** (70 Rust + 14 frontend) covering critical paths across the CLI, shared data model, Tauri config generation, client-config write logic, Docker sandbox generation, AES-256-GCM vault, and frontend utilities. All green on macOS.

### Rust tests (run with `cargo test --workspace`)

| Suite | Count | File | What it proves |
|-------|-------|------|----------------|
| `mcp-proxy-common` unit | 9 | [crates/mcp-proxy-common/src/models.rs](crates/mcp-proxy-common/src/models.rs), [local_backend.rs](crates/mcp-proxy-common/src/local_backend.rs) | SecretSource serde round-trip + legacy `Keychain`/`EncryptedFile` alias; default server config; platform-correct local backend selection; macOS-specific `is_unlocked`/`unlock`/`lock` no-op behavior |
| `mcp-proxy-common` vault | 12 | [crates/mcp-proxy-common/src/vault.rs](crates/mcp-proxy-common/src/vault.rs) | AES-GCM + Argon2id roundtrip; wrong password → generic WrongPasswordOrCorrupted (no info leak); tampered ciphertext / tampered salt / wrong magic / unknown version / truncated file all fail cleanly; fresh nonce per write; create refuses to overwrite; idempotent delete; multi-entry |
| `mcp-proxy-cli` docker unit | 13 | [crates/mcp-proxy-cli/src/docker.rs](crates/mcp-proxy-cli/src/docker.rs) | Dockerfile has two stages / references user image / uses agent entrypoint; image tag is deterministic, content-hashed over image+command+args+agent source, independent of env_vars+extra_args; server-id sanitizer; build context writes Dockerfile + embedded agent source; rebuild is idempotent |
| `mcp-proxy-cli` integration | 10 | [crates/mcp-proxy-cli/tests/cli_integration.rs](crates/mcp-proxy-cli/tests/cli_integration.rs) | `list`/`run`/`--help`/`--version`, all error paths (unknown/disabled/Docker-without-image/missing-secret/no-servers.json), success path |
| `mcp-proxy-cli` E2E stdio | 2 | [crates/mcp-proxy-cli/tests/e2e_stdio_pipe.rs](crates/mcp-proxy-cli/tests/e2e_stdio_pipe.rs) | JSON-RPC `initialize` round-trips through `/bin/cat`; child inherits `PATH` |
| `mcp-proxy` (src-tauri) config | 6 | [src-tauri/src/commands/config.rs](src-tauri/src/commands/config.rs) | Multi-client config shape (Claude/Cursor/Codex/VS Code/Windsurf) + regression guard that no secret values ever leak into generated configs |
| `mcp-proxy` (src-tauri) client_write | 18 | [src-tauri/src/commands/client_write.rs](src-tauri/src/commands/client_write.rs) | Path resolution per client; JSON + TOML merge preserve user entries / replace stale mcp-proxy entries / reject malformed input; atomic write with timestamped backup; `vscode` is unsupported with a message; entries builders use `mcp-proxy` marker |

### Frontend tests (run with `npm test`)

| Suite | Count | File | What it proves |
|-------|-------|------|----------------|
| `registry` data + filter | 14 | [src/data/\_\_tests\_\_/registry.test.ts](src/data/__tests__/registry.test.ts) | Unique IDs / non-empty publisher / env var spec integrity; `filterEntries` works for region + name + publisher + tag + id + description; case-insensitive; whitespace-safe; cross-region isolation |

### Previously verified manually (now covered by automation)

- CLI `list` and `run` behavior, all error paths ✅ integration tests
- stdio transparency (the core MCP transport promise) ✅ E2E tests
- SecretSource backward-compat with legacy on-disk tags ✅ unit tests
- Workspace + frontend still build (`cargo check --workspace`, `npm run build`) ✅ CI-ready

### Still manual (see [docs/e2e-manual.md](docs/e2e-manual.md))

- Real AI client integration (Claude Desktop, Codex, Cursor, VS Code, Windsurf) — cannot be automated because they're GUI apps
- Real secret injection with API keys (Brave Search, GitHub PAT, etc.) against live MCP servers

### Still NOT tested (gaps)

- **Tauri commands** — `add_server`, `set_secret`, `delete_secret`, `start_proxy` (desktop mode) — not yet covered
- **React component tests** — pages + UI components; only pure utilities (`filterEntries`) are tested so far
- **Cross-platform** — only macOS runs the test suite; Linux/Windows `Local` backend is a stub
- **Proxy lifecycle** — long-running proxy with real MCP traffic; covered by manual checklist only
- **Concurrent proxies** — multiple servers started simultaneously
- **Docker sandbox** — not implemented in CLI
- **EncryptedFile backend** — stub on non-macOS platforms

---

## 2. Principles

1. **Test what matters, not what's easy.** Prioritize security boundaries (secret handling), data-model contracts (serde), and the critical user path (CLI `run` flow). Skip coverage for trivial getters/setters.
2. **Cheapest effective test wins.** Prefer `cargo test` unit tests (sub-second) over integration tests; integration tests over end-to-end; end-to-end over manual.
3. **One assertion, one failure.** Each test checks one behavior so failures point to a specific bug.
4. **Tests are documentation.** A well-named test like `secret_source_serde_accepts_legacy_keychain_tag` tells future readers both the intent and the contract.
5. **Don't test implementation details.** Test observable behavior (JSON shape, env vars in child process, HTTP responses), not which function called which.
6. **Fast feedback in dev, thorough in CI.** `cargo test -p mcp-proxy-common` runs in under a second locally; full E2E can be gated on pre-merge CI only.

---

## 3. Mandatory Test Rules

When submitting a change, these tests **must** exist (add them as part of the same PR):

### 3.1 Data model changes → serde round-trip + alias test

Any change to `mcp_proxy_common::models` (adding/removing enum variants, renaming fields, adding `#[serde(alias)]`) requires a round-trip test:

```rust
#[test]
fn secret_source_round_trips() {
    let source = SecretSource::OnePassword { reference: "op://a/b/c".into() };
    let json = serde_json::to_string(&source).unwrap();
    let back: SecretSource = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, SecretSource::OnePassword { .. }));
}
```

If you add a `#[serde(alias = "OldName")]`, also add a test that deserializes the old tag (see [crates/mcp-proxy-common/src/models.rs](crates/mcp-proxy-common/src/models.rs) for the canonical example).

### 3.2 Secret resolution changes → unit test with mocked backend

Changes to `secret_resolver.rs` or `local_backend.rs` must include a unit test. Keychain/1Password side effects can be mocked or gated behind `#[cfg(test)]`:

- **Pure logic** (e.g., which backend is selected on each platform): use `#[cfg(target_os = ...)]` and assert `default_backend()` returns the expected variant.
- **Error paths** (e.g., `op` CLI missing): inject a `Command` abstraction or use `#[cfg(test)]` to override the resolver.

### 3.3 CLI argument / behavior changes → integration test with fixtures

Changes to `mcp-proxy run` or `list` must add an integration test under `crates/mcp-proxy-cli/tests/`:

- Set up temp `servers.json` + `secrets_meta.json` in a tempdir
- Point CLI at tempdir via an env var (add one if not present: `MCP_PROXY_DATA_DIR`)
- Assert on stdout, stderr, exit code, and any spawned process's env vars

Example skeleton:

```rust
// crates/mcp-proxy-cli/tests/run_command.rs
use std::process::Command;
use tempfile::TempDir;

#[test]
fn run_injects_env_var_from_local_secret() {
    let tmp = TempDir::new().unwrap();
    // ... write fixtures, set MCP_PROXY_DATA_DIR, spawn CLI, assert
}
```

### 3.4 Security-relevant code → explicit test

Any change to code that handles secret VALUES (not references) requires at least one test proving:
- The secret value never appears in logs (capture stderr, assert absence)
- The secret is zeroed from memory after use where `zeroize` is applicable
- On error paths, the secret is not leaked via the error message

### 3.5 UI layout changes → screenshot verification

Visual changes to components used in multiple pages (`Modal`, `PillButton`, `Card`, etc.) must be verified via Claude Preview screenshots of at least one page where the component is used. Record before/after in the PR description.

For font-size or padding changes, also capture a `preview_inspect` of the computed `height`/`padding` to prove no regression.

### 3.6 New Tauri command → smoke test

Adding a `#[tauri::command]` fn requires at least:
- A Rust unit test for the pure logic it delegates to
- A frontend `tauri.ts` wrapper with matching TypeScript types
- Manual verification in `cargo tauri dev` that the command returns successfully

---

## 4. Test Categories

### Rust unit tests (preferred default)

Location: `#[cfg(test)] mod tests { ... }` at the bottom of the file under test.

Use for: data models, pure functions, small logic blocks.

Run: `cargo test -p <crate-name>` or `cargo test --workspace`.

### Rust integration tests

Location: `crates/<crate>/tests/*.rs`.

Use for: CLI end-to-end, multi-module flows, anything that spawns processes.

Dependencies: add `tempfile`, `assert_cmd`, `predicates` as dev-deps when needed.

### Frontend tests

**Not yet configured.** When the first frontend test is written, configure Vitest (`vitest` + `@testing-library/react`) and add an `npm test` script. Keep React component tests focused on behavior (clicks, state changes) rather than rendered HTML.

### End-to-end (manual or scripted)

Location: `scripts/e2e/*.sh` (doesn't exist yet — create when needed).

Use for: real AI client integration, real MCP server traffic, cross-platform sanity checks.

---

## 5. Running Tests

```bash
# Rust — all crates (70 tests: unit + integration + E2E + config + client_write + docker + vault)
cargo test --workspace

# Rust — single crate
cargo test -p mcp-proxy-common           # 9 unit + 12 vault = 21 tests
cargo test -p mcp-proxy-cli              # 13 docker unit + 10 integ + 2 E2E = 25
cargo test -p mcp-proxy                  # 6 config + 18 client_write = 24 tests

# Docker sandbox integration test (requires Docker; ~3 min first run)
cargo test -p mcp-proxy-cli -- --ignored docker

# Rust — with output visible (prints, eprintln, tracing)
cargo test --workspace -- --nocapture

# Frontend — Vitest (14 tests, sub-second)
npm test            # run once
npm run test:watch  # watch mode for dev

# Compile check only (fastest, no tests executed)
cargo check --workspace

# Frontend type check + build
npm run build

# Frontend dev-mode smoke test
npm run dev  # then open http://localhost:1420

# CLI binary smoke test
cargo build -p mcp-proxy-cli --release
./target/release/mcp-proxy --version
./target/release/mcp-proxy list
```

### Data directory override for tests & profiles

The `MCP_PROXY_DATA_DIR` env var overrides the default `~/Library/Application Support/com.mcp-proxy.app` location. Integration tests use it to isolate fixtures in temp dirs. Power users can use it to keep multiple independent profiles:

```bash
MCP_PROXY_DATA_DIR=/tmp/mcp-proxy-sandbox ./target/release/mcp-proxy list
```

### Manual end-to-end verification

Automated tests cover the CLI and transport. Real AI client integration is documented step-by-step in [docs/e2e-manual.md](docs/e2e-manual.md) — run through it before each release.

---

## 6. Pre-merge Checklist

Before merging a change, verify:

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes (or N/A if no test lands)
- [ ] `npm run build` passes
- [ ] New code paths have tests per the rules in §3
- [ ] Manual smoke test of the user-facing flow (e.g., "add a secret, see it in the list")
- [ ] No secret values appear in git diffs (`.env`, `credentials.json`, tokens pasted in docs)

---

## 7. CI/CD

GitHub Actions workflow at [.github/workflows/ci.yml](.github/workflows/ci.yml) runs on every push and pull request to `main`:

**Rust job** (macos-latest, because `keyring` needs `Security.framework`):
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo build -p mcp-proxy-cli --release` + `--version` / `--help` smoke

**Frontend job** (ubuntu-latest):
1. `npm ci`
2. `npm run build` (TypeScript strict + Vite)
3. `npm test` (Vitest)

Concurrency is set to cancel older runs of the same branch when a new push arrives.

**Not yet in CI**:
- Playwright / Tauri in-app end-to-end test (needs a headful / Xvfb setup)
- `cargo test -p mcp-proxy-cli -- --ignored docker` (requires Docker in the runner)

---

## 8. Flaky Test Policy

A test that fails once out of 20 runs is banned from the suite — either fix it or delete it. Flaky tests erode trust faster than missing tests. If a test genuinely needs external resources (network, Docker, real Keychain), mark it with `#[ignore]` and document how to opt in via an env var.

---

## 9. What to Test Next (priority order)

1. ✅ ~~`SecretSource` serde alias test~~ — [crates/mcp-proxy-common/src/models.rs](crates/mcp-proxy-common/src/models.rs)
2. ✅ ~~`local_backend::default_backend()` per-platform test~~ — [crates/mcp-proxy-common/src/local_backend.rs](crates/mcp-proxy-common/src/local_backend.rs)
3. ✅ ~~CLI `run`/`list` integration tests with `MCP_PROXY_DATA_DIR`~~ — [crates/mcp-proxy-cli/tests/cli_integration.rs](crates/mcp-proxy-cli/tests/cli_integration.rs)
4. ✅ ~~stdio transparency E2E~~ — [crates/mcp-proxy-cli/tests/e2e_stdio_pipe.rs](crates/mcp-proxy-cli/tests/e2e_stdio_pipe.rs)
5. ✅ ~~Tauri `generate_config` for each client~~ — [src-tauri/src/commands/config.rs](src-tauri/src/commands/config.rs)
6. ✅ ~~`filterEntries()` frontend unit test + Vitest bootstrap~~ — [src/data/\_\_tests\_\_/registry.test.ts](src/data/__tests__/registry.test.ts)
7. **Tauri `add_server` / `set_secret` / `delete_secret` unit tests** — use `State<AppState>` with fixture state; requires figuring out `tauri::test::mock_app()` or similar.
8. **`generate_config` integration through the Tauri command** — today only the pure helpers are tested; cover the `State<AppState>`-taking async command too.
9. **React component tests** — start with Modal (open/close/ESC) and the Env Mapping editor (add/remove rows). Needs `@testing-library/react` + jsdom.
10. **Zustand store tests** — `useServers` / `useSecrets` with mocked `invoke`; catches logic bugs in optimistic updates and error handling.
11. ✅ ~~CI pipeline~~ — [.github/workflows/ci.yml](.github/workflows/ci.yml) runs fmt + clippy + test + build on every push / PR to `main`.
12. **Docker sandbox integration test** — currently gated behind `#[ignore]`. Enable in a CI job that has Docker, or write as a shell script under `scripts/` so it runs on manual request.
