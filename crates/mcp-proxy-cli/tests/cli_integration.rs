//! Integration tests for the `mcp-proxy` CLI.
//!
//! Each test runs the real release-like binary (via `CARGO_BIN_EXE_mcp-proxy`)
//! against a temporary data directory populated with fixture JSON files.
//! No real Keychain, 1Password, or `op` CLI is touched.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Fixture + process harness shared across tests.
struct TestEnv {
    data_dir: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        Self {
            data_dir: TempDir::new().expect("create tempdir"),
        }
    }

    fn path(&self) -> &Path {
        self.data_dir.path()
    }

    /// Write a `servers.json` fixture. Accepts a `serde_json::Value` for flexibility.
    fn write_servers(&self, value: serde_json::Value) {
        fs::write(
            self.path().join("servers.json"),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
    }

    /// Write a `secrets_meta.json` fixture.
    #[allow(dead_code)]
    fn write_secrets_meta(&self, value: serde_json::Value) {
        fs::write(
            self.path().join("secrets_meta.json"),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
    }

    /// Start an `assert_cmd` Command pointed at this test's data directory.
    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("mcp-proxy").unwrap();
        cmd.env("MCP_PROXY_DATA_DIR", self.path());
        // Silence tracing output — tests assert on stderr shape
        cmd.env("RUST_LOG", "off");
        cmd
    }
}

/// Minimal server fixture helper — only the fields the CLI actually reads.
fn server_fixture(
    id: &str,
    command: &str,
    args: Vec<&str>,
    enabled: bool,
    run_mode: serde_json::Value,
    env_mappings: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "id": id,
        "name": id,
        "command": command,
        "args": args,
        "transport": { "type": "Stdio" },
        "env_mappings": env_mappings,
        "run_mode": run_mode,
        "enabled": enabled,
        "trusted": false,
        "created_at": "2026-04-17T00:00:00Z",
        "updated_at": "2026-04-17T00:00:00Z",
    })
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[test]
fn list_empty_store_shows_placeholder() {
    let env = TestEnv::new();
    env.cmd()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No MCP servers configured"));
}

#[test]
fn list_shows_configured_servers() {
    let env = TestEnv::new();
    env.write_servers(json!([
        server_fixture(
            "github",
            "npx",
            vec!["-y", "@github/github-mcp-server"],
            true,
            json!({ "type": "Local" }),
            vec![json!({ "env_var_name": "GITHUB_TOKEN", "secret_ref": "gh-pat" }),],
        ),
        server_fixture(
            "sandbox",
            "npx",
            vec!["-y", "@example/x"],
            false,
            json!({ "type": "DockerSandbox", "image": null, "extra_args": [] }),
            vec![],
        ),
    ]));

    env.cmd()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("github"))
        .stdout(predicate::str::contains("enabled, local"))
        .stdout(predicate::str::contains("sandbox"))
        .stdout(predicate::str::contains("disabled, docker"))
        .stdout(predicate::str::contains("GITHUB_TOKEN → secret:gh-pat"));
}

// ---------------------------------------------------------------------------
// run — error paths
// ---------------------------------------------------------------------------

#[test]
fn run_unknown_server_fails_with_message() {
    let env = TestEnv::new();
    env.write_servers(json!([]));

    env.cmd()
        .args(["run", "ghost"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("ghost"))
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn run_without_servers_json_fails_with_message() {
    // No servers.json written at all — CLI should explain the problem clearly.
    let env = TestEnv::new();
    env.cmd()
        .args(["run", "whatever"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("servers.json"));
}

#[test]
fn run_disabled_server_fails_with_message() {
    let env = TestEnv::new();
    env.write_servers(json!([server_fixture(
        "sleeping",
        "/bin/sh",
        vec!["-c", "exit 0"],
        false,
        json!({ "type": "Local" }),
        vec![],
    )]));

    env.cmd()
        .args(["run", "sleeping"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("disabled"));
}

#[test]
fn run_docker_mode_without_image_rejects_with_help() {
    // Docker sandbox mode is implemented, but the user must specify a base
    // image. With `image: null` the CLI refuses cleanly and explains.
    let env = TestEnv::new();
    env.write_servers(json!([server_fixture(
        "boxed",
        "/bin/sh",
        vec!["-c", "exit 0"],
        true,
        json!({ "type": "DockerSandbox", "image": null, "extra_args": [] }),
        vec![],
    )]));

    env.cmd()
        .args(["run", "boxed"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("base image"))
        .stderr(predicate::str::contains("server config"));
}

#[test]
fn run_missing_secret_reference_fails() {
    let env = TestEnv::new();
    env.write_servers(json!([server_fixture(
        "needs-secret",
        "/bin/sh",
        vec!["-c", "exit 0"],
        true,
        json!({ "type": "Local" }),
        vec![json!({ "env_var_name": "API_KEY", "secret_ref": "nonexistent" }),],
    )]));
    // Deliberately don't write secrets_meta.json.

    env.cmd()
        .args(["run", "needs-secret"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("nonexistent"))
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// run — success path
// ---------------------------------------------------------------------------

#[test]
fn run_local_no_mappings_succeeds() {
    let env = TestEnv::new();
    env.write_servers(json!([server_fixture(
        "quick-exit",
        "/bin/sh",
        vec!["-c", "exit 0"],
        true,
        json!({ "type": "Local" }),
        vec![],
    )]));

    env.cmd().args(["run", "quick-exit"]).assert().success();
}

// ---------------------------------------------------------------------------
// flags
// ---------------------------------------------------------------------------

#[test]
fn version_flag_prints_version() {
    Command::cargo_bin("mcp-proxy")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("mcp-proxy"))
        .stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn help_flag_lists_subcommands() {
    Command::cargo_bin("mcp-proxy")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("list"));
}
