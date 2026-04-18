//! Integration test for the macOS `sandbox-exec` wrapper on Local-mode children.
//!
//! The whole file is gated on macOS — on other platforms the sandbox module is
//! a no-op and this test contributes nothing.

#![cfg(target_os = "macos")]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Write a `servers.json` fixture with a single trusted, `sandbox_local = true`
/// Local-mode server that runs `/usr/bin/true` (exit 0, no stdio).
fn write_trivial_servers(dir: &std::path::Path, command: &str, args: Vec<&str>) {
    let servers = json!([{
        "id": "sbx",
        "name": "sbx",
        "command": command,
        "args": args,
        "transport": { "type": "Stdio" },
        "env_mappings": [],
        "run_mode": { "type": "Local" },
        "enabled": true,
        "trusted": true,
        "sandbox_local": true,
        "created_at": "2026-04-17T00:00:00Z",
        "updated_at": "2026-04-17T00:00:00Z",
    }]);
    fs::write(
        dir.join("servers.json"),
        serde_json::to_string_pretty(&servers).unwrap(),
    )
    .unwrap();
}

#[test]
fn sandbox_local_runs_trivial_child_to_exit_zero() {
    // `/usr/bin/true` is guaranteed on every supported macOS release and runs
    // cleanly under `(deny default)` with only the profile's `process-exec*` +
    // libSystem-friendly allows — the most minimal smoke test for the wrapper.
    let tmp = TempDir::new().expect("tempdir");
    write_trivial_servers(tmp.path(), "/usr/bin/true", vec![]);

    let mut cmd = Command::cargo_bin("mcp-proxy").unwrap();
    cmd.env("MCP_PROXY_DATA_DIR", tmp.path())
        .env("RUST_LOG", "off")
        .args(["run", "sbx"]);

    cmd.assert().success();
}

#[test]
fn sandbox_local_child_can_print_to_stdout() {
    // /bin/echo is also present on every macOS. Verifies that stdio inheritance
    // survives the sandbox-exec wrapper — if the profile blocked /dev/stdout
    // writes this would fail.
    let tmp = TempDir::new().expect("tempdir");
    write_trivial_servers(tmp.path(), "/bin/echo", vec!["hello-sbx"]);

    let mut cmd = Command::cargo_bin("mcp-proxy").unwrap();
    cmd.env("MCP_PROXY_DATA_DIR", tmp.path())
        .env("RUST_LOG", "info")
        .args(["run", "sbx"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello-sbx"))
        // Stderr should mention wrapping in sandbox-exec at info level.
        .stderr(predicate::str::contains("sandbox-exec"));
}
