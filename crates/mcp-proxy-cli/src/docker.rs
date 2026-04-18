//! Docker sandbox runtime for `mcp-proxy run`.
//!
//! Architecture (CLAUDE.md "Docker Sandbox"):
//!
//! 1. Generate a multi-stage Dockerfile that compiles `mcp-proxy-agent` from
//!    embedded source in a `rust:alpine` stage, then copies it into the user's
//!    chosen runtime image and sets ENTRYPOINT to the agent.
//! 2. `docker build -t <tag>` — cached by a content hash over all inputs, so
//!    builds only rerun when the config actually changes.
//! 3. `docker run -i --rm <tag>` with secrets delivered as **the first line of
//!    stdin** (JSON `SecretPayload`). The agent reads that line, injects env
//!    vars, and `exec()`s the real MCP server. Subsequent stdin/stdout is
//!    pure MCP traffic, flowing transparently from the AI client through our
//!    CLI into the container.
//!
//! Secrets never reach the command line, env file, image layer, or
//! `docker inspect` output.

use mcp_proxy_common::invocation_log::{Direction, InvocationLogger, LoggerHandle};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Resolve the `docker` binary to invoke. Defaults to `docker` on `PATH`.
/// Overridable via `MCP_PROXY_DOCKER_BIN` for two reasons: (1) unusual host
/// setups where the docker CLI lives somewhere odd, and (2) tests that point
/// at a fake `docker` shell script so they can run without a real Docker
/// daemon.
fn docker_bin() -> OsString {
    std::env::var_os("MCP_PROXY_DOCKER_BIN").unwrap_or_else(|| OsString::from("docker"))
}

/// Embedded agent source — baked into the CLI binary at compile time so
/// deployed binaries are self-contained (no workspace lookup at runtime).
const AGENT_CARGO_TOML: &str = include_str!("../../mcp-proxy-agent/Cargo.toml");
const AGENT_MAIN_RS: &str = include_str!("../../mcp-proxy-agent/src/main.rs");

/// What the host needs to know to run a server in a Docker sandbox.
pub struct SandboxConfig<'a> {
    pub server_id: &'a str,
    pub image: &'a str,
    pub command: &'a str,
    pub args: &'a [String],
    pub env_vars: &'a HashMap<String, String>,
    pub extra_args: &'a [String],
    /// Whether the operator has reviewed and trusted this server. Untrusted
    /// servers get `--network=none` injected by default (unless the operator
    /// also set `--network` explicitly in `extra_args`). See the trust gate
    /// in `main.rs::run_server` — untrusted servers without an explicit
    /// network override never reach this function.
    pub trusted: bool,
    /// Where build contexts get cached. Typically
    /// `mcp_proxy_common::store::app_data_dir().join("docker-build")`.
    pub build_root: &'a Path,
}

/// JSON shape written to the container's stdin as its first line — must match
/// the agent's parser in `crates/mcp-proxy-agent/src/main.rs`.
#[derive(serde::Serialize)]
struct SecretPayload<'a> {
    env_vars: &'a HashMap<String, String>,
    command: &'a str,
    args: &'a [String],
}

/// Entry point called from `main.rs` when `RunMode::DockerSandbox`.
/// Builds if needed, runs, shuttles stdio, waits, propagates exit code.
pub fn run_sandbox(cfg: SandboxConfig, logger: Option<&InvocationLogger>) -> Result<i32, String> {
    let bin = docker_bin();
    ensure_docker_available(&bin)?;

    let tag = compute_image_tag(&cfg);

    if !image_exists(&bin, &tag)? {
        let ctx_dir = cfg.build_root.join(sanitize_component(cfg.server_id));
        write_build_context(&ctx_dir, cfg.image)?;
        docker_build(&bin, &ctx_dir, &tag)?;
    }

    let payload = SecretPayload {
        env_vars: cfg.env_vars,
        command: cfg.command,
        args: cfg.args,
    };

    docker_run_with_stdin_payload(
        &bin,
        &tag,
        cfg.extra_args,
        cfg.trusted,
        &payload,
        logger.and_then(|l| l.handle()),
    )
}

// ---------------------------------------------------------------------------
// network policy
// ---------------------------------------------------------------------------

/// Returns `true` if `extra_args` already contains a `--network` / `--net`
/// flag (either `--network=foo` or `--network foo`). The operator's explicit
/// choice always wins over our default.
pub(crate) fn extra_args_specify_network(extra_args: &[String]) -> bool {
    extra_args.iter().any(|a| {
        a == "--network" || a == "--net" || a.starts_with("--network=") || a.starts_with("--net=")
    })
}

/// Network flag we inject into `docker run` based on trust. Returns `None`
/// when the operator already specified a `--network` flag in `extra_args`
/// (their choice wins), or when the server is trusted (we leave Docker's
/// default bridge network alone — matches local-mode behaviour).
///
/// Untrusted servers get `--network=none` to prevent secret exfiltration by
/// a compromised or malicious MCP server. Most real MCP servers need network
/// access, so the UI / docs steer users toward marking the server trusted
/// once they've reviewed it.
pub(crate) fn resolve_network_flag(trusted: bool, extra_args: &[String]) -> Option<&'static str> {
    if extra_args_specify_network(extra_args) {
        return None;
    }
    if trusted {
        None
    } else {
        Some("--network=none")
    }
}

// ---------------------------------------------------------------------------
// log driver policy
// ---------------------------------------------------------------------------

/// Returns `true` if `extra_args` already contains a `--log-driver` flag
/// (either `--log-driver=foo` or `--log-driver foo`). The operator's explicit
/// choice always wins over our default.
pub(crate) fn extra_args_specify_log_driver(extra_args: &[String]) -> bool {
    extra_args
        .iter()
        .any(|a| a == "--log-driver" || a.starts_with("--log-driver="))
}

/// Log-driver flag we inject into `docker run`. Defaults to `--log-driver=none`
/// so that operators who have configured a non-default Docker log driver
/// (e.g. `journald`, `fluentd`, `splunk`, `gelf`) do not accidentally persist
/// the one-line JSON secret payload we write to container stdin. Docker's
/// default `json-file` driver does not capture stdin, so this is defense in
/// depth — but the cost of being wrong is leaking secrets to a log sink, so
/// we inject unconditionally unless the operator opted out.
///
/// Returns `None` if the operator already supplied `--log-driver` in
/// `extra_args`, letting their choice win.
pub(crate) fn resolve_log_driver_flag(extra_args: &[String]) -> Option<&'static str> {
    if extra_args_specify_log_driver(extra_args) {
        None
    } else {
        Some("--log-driver=none")
    }
}

// ---------------------------------------------------------------------------
// docker CLI detection
// ---------------------------------------------------------------------------

fn ensure_docker_available(bin: &OsStr) -> Result<(), String> {
    match Command::new(bin).arg("--version").output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!(
            "`docker --version` failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        )),
        Err(_) => Err(
            "Docker is not installed or not on PATH. Install Docker Desktop and retry.".to_string(),
        ),
    }
}

// ---------------------------------------------------------------------------
// tag + sanitizer
// ---------------------------------------------------------------------------

fn compute_image_tag(cfg: &SandboxConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cfg.image.as_bytes());
    hasher.update(b"\0");
    hasher.update(cfg.command.as_bytes());
    hasher.update(b"\0");
    for a in cfg.args {
        hasher.update(a.as_bytes());
        hasher.update(b"\0");
    }
    // Include the embedded agent source so a CLI upgrade that changes the
    // agent invalidates prior images automatically.
    hasher.update(AGENT_CARGO_TOML.as_bytes());
    hasher.update(AGENT_MAIN_RS.as_bytes());

    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    format!(
        "mcp-proxy-local/{}:{}",
        sanitize_component(cfg.server_id),
        hex
    )
}

/// Make an ID safe for Docker repo / path components: lowercase, keep
/// `[a-z0-9]`, replace anything else with `-`, strip leading/trailing `-`.
fn sanitize_component(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("server");
    }
    out
}

// ---------------------------------------------------------------------------
// build context
// ---------------------------------------------------------------------------

fn dockerfile_contents(user_image: &str) -> String {
    format!(
        r#"# syntax=docker/dockerfile:1
# --- Stage 1: build mcp-proxy-agent from embedded source ----------
FROM rust:1.80-alpine AS agent-builder
RUN apk add --no-cache musl-dev
WORKDIR /build/agent-src
COPY agent-src/Cargo.toml ./Cargo.toml
COPY agent-src/src ./src
RUN cargo build --release

# --- Stage 2: user's runtime image --------------------------------
FROM {user_image}
COPY --from=agent-builder /build/agent-src/target/release/mcp-proxy-agent /usr/local/bin/mcp-proxy-agent
ENTRYPOINT ["/usr/local/bin/mcp-proxy-agent"]
"#
    )
}

fn write_build_context(ctx_dir: &Path, user_image: &str) -> Result<(), String> {
    // Clean + recreate so stale state from previous runs can't confuse docker build
    if ctx_dir.exists() {
        fs::remove_dir_all(ctx_dir)
            .map_err(|e| format!("Failed to clean build context {}: {e}", ctx_dir.display()))?;
    }
    let agent_src_dir = ctx_dir.join("agent-src").join("src");
    fs::create_dir_all(&agent_src_dir)
        .map_err(|e| format!("Failed to create build context: {e}"))?;

    fs::write(
        ctx_dir.join("agent-src").join("Cargo.toml"),
        AGENT_CARGO_TOML,
    )
    .map_err(|e| format!("Failed to write agent-src/Cargo.toml: {e}"))?;

    fs::write(agent_src_dir.join("main.rs"), AGENT_MAIN_RS)
        .map_err(|e| format!("Failed to write agent-src/src/main.rs: {e}"))?;

    fs::write(ctx_dir.join("Dockerfile"), dockerfile_contents(user_image))
        .map_err(|e| format!("Failed to write Dockerfile: {e}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// docker image ops
// ---------------------------------------------------------------------------

fn image_exists(bin: &OsStr, tag: &str) -> Result<bool, String> {
    let output = Command::new(bin)
        .args(["image", "inspect", tag])
        .output()
        .map_err(|e| format!("Failed to invoke `docker image inspect`: {e}"))?;
    Ok(output.status.success())
}

fn docker_build(bin: &OsStr, ctx_dir: &Path, tag: &str) -> Result<(), String> {
    eprintln!("mcp-proxy: building sandbox image {tag} (first build may take ~2 min)");
    let status = Command::new(bin)
        .args(["build", "-t", tag])
        .arg(ctx_dir)
        // Inherit stderr/stdout so the user sees docker build output in real time
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to invoke `docker build`: {e}"))?;
    if !status.success() {
        return Err(format!(
            "`docker build` failed with exit code {:?}. See build output above.",
            status.code()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// docker run + stdio shuttle
// ---------------------------------------------------------------------------

/// Pure argv construction for `docker run …` — extracted so tests can assert
/// the exact flag order without spinning up a Docker daemon.
pub(crate) fn build_run_argv(tag: &str, extra_args: &[String], trusted: bool) -> Vec<String> {
    let mut argv = vec!["run".to_string(), "-i".to_string(), "--rm".to_string()];
    if let Some(flag) = resolve_log_driver_flag(extra_args) {
        argv.push(flag.to_string());
    }
    if let Some(flag) = resolve_network_flag(trusted, extra_args) {
        argv.push(flag.to_string());
    }
    argv.extend(extra_args.iter().cloned());
    argv.push(tag.to_string());
    argv
}

fn docker_run_with_stdin_payload(
    bin: &OsStr,
    tag: &str,
    extra_args: &[String],
    trusted: bool,
    payload: &SecretPayload,
    logger: Option<LoggerHandle>,
) -> Result<i32, String> {
    let mut cmd = Command::new(bin);
    cmd.args(build_run_argv(tag, extra_args, trusted));
    // When logging is on, pipe stdout so we can tee JSON-RPC responses too.
    let tee = logger.is_some();
    cmd.stdin(Stdio::piped())
        .stdout(if tee {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to invoke `docker run`: {e}"))?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "docker run: failed to acquire stdin pipe".to_string())?;
    {
        let line = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize secret payload: {e}"))?;
        child_stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("Failed to write secret payload to container stdin: {e}"))?;
        child_stdin
            .write_all(b"\n")
            .map_err(|e| format!("Failed to write payload terminator: {e}"))?;
        child_stdin
            .flush()
            .map_err(|e| format!("Failed to flush payload to container stdin: {e}"))?;
    }

    // After the secret line, the rest of the host's stdin is the AI client's
    // MCP traffic. When logging is on, tee it line-by-line so we can record
    // requests; otherwise just bulk-copy bytes (original behavior).
    let pump = if let Some(h) = logger.clone() {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            let host_stdin = std::io::stdin();
            let mut reader = BufReader::new(host_stdin.lock());
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
                let is_req = serde_json::from_str::<serde_json::Value>(line.trim())
                    .ok()
                    .and_then(|v| v.get("id").cloned())
                    .is_some();
                h.record_line(
                    if is_req {
                        Direction::Request
                    } else {
                        Direction::Notification
                    },
                    line.trim_end_matches('\n'),
                );
                if child_stdin.write_all(line.as_bytes()).is_err() {
                    break;
                }
                let _ = child_stdin.flush();
            }
        })
    } else {
        std::thread::spawn(move || {
            let mut host_stdin = std::io::stdin();
            let _ = std::io::copy(&mut host_stdin, &mut child_stdin);
        })
    };

    // child stdout → host stdout (tee'd only when logger is on)
    if let Some(h) = logger {
        let child_stdout = child.stdout.take();
        if let Some(mut out) = child_stdout {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Read, Write};
                let _ = &mut out as &mut dyn Read;
                let mut reader = BufReader::new(out);
                let mut line = String::new();
                let stdout = std::io::stdout();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {}
                        Err(_) => break,
                    }
                    h.record_line(Direction::Response, line.trim_end_matches('\n'));
                    let mut lock = stdout.lock();
                    if lock.write_all(line.as_bytes()).is_err() {
                        break;
                    }
                    let _ = lock.flush();
                }
            });
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait on container: {e}"))?;
    let _ = pump.join();

    Ok(status
        .code()
        .unwrap_or(if status.success() { 0 } else { 1 }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn sample_cfg<'a>(
        build_root: &'a Path,
        env_vars: &'a HashMap<String, String>,
        image: &'a str,
        command: &'a str,
        args: &'a [String],
        extra_args: &'a [String],
    ) -> SandboxConfig<'a> {
        SandboxConfig {
            server_id: "my-server",
            image,
            command,
            args,
            env_vars,
            extra_args,
            trusted: true,
            build_root,
        }
    }

    // --- Network policy --------------------------------------------------

    #[test]
    fn network_flag_untrusted_with_no_override_defaults_to_none() {
        let extra: Vec<String> = vec![];
        assert_eq!(resolve_network_flag(false, &extra), Some("--network=none"));
    }

    #[test]
    fn network_flag_trusted_leaves_bridge_default() {
        let extra: Vec<String> = vec![];
        assert_eq!(resolve_network_flag(true, &extra), None);
    }

    #[test]
    fn network_flag_respects_explicit_network_equals_override() {
        // Untrusted + explicit override → we yield to the user's choice.
        let extra = vec!["--network=host".to_string()];
        assert_eq!(resolve_network_flag(false, &extra), None);
    }

    #[test]
    fn network_flag_respects_explicit_network_space_override() {
        // `--network bridge` as two tokens must also count as explicit.
        let extra = vec!["--network".to_string(), "bridge".to_string()];
        assert_eq!(resolve_network_flag(false, &extra), None);
    }

    #[test]
    fn network_flag_respects_explicit_net_alias() {
        let extra = vec!["--net=none".to_string()];
        assert!(extra_args_specify_network(&extra));
        assert_eq!(resolve_network_flag(false, &extra), None);
    }

    #[test]
    fn network_flag_respects_short_net_two_token() {
        let extra = vec!["--net".to_string(), "host".to_string()];
        assert!(extra_args_specify_network(&extra));
    }

    #[test]
    fn network_flag_ignores_unrelated_args() {
        // `--networkless=foo` etc. must not be treated as a network override.
        let extra = vec![
            "-v".to_string(),
            "/tmp:/tmp".to_string(),
            "--memory=512m".to_string(),
        ];
        assert!(!extra_args_specify_network(&extra));
        assert_eq!(resolve_network_flag(false, &extra), Some("--network=none"));
    }

    // --- Log driver policy -----------------------------------------------

    #[test]
    fn log_driver_flag_defaults_to_none() {
        let extra: Vec<String> = vec![];
        assert_eq!(resolve_log_driver_flag(&extra), Some("--log-driver=none"));
    }

    #[test]
    fn log_driver_flag_respects_explicit_equals_override() {
        // `--log-driver=json-file` as a single token must win.
        let extra = vec!["--log-driver=json-file".to_string()];
        assert!(extra_args_specify_log_driver(&extra));
        assert_eq!(resolve_log_driver_flag(&extra), None);
    }

    #[test]
    fn log_driver_flag_respects_explicit_space_override() {
        // `--log-driver foo` as two tokens must also count as explicit.
        let extra = vec!["--log-driver".to_string(), "foo".to_string()];
        assert!(extra_args_specify_log_driver(&extra));
        assert_eq!(resolve_log_driver_flag(&extra), None);
    }

    #[test]
    fn log_driver_flag_ignores_unrelated_args() {
        // Unrelated flags (including `--log-opt`) must not be treated as an
        // override of `--log-driver`.
        let extra = vec![
            "-v".to_string(),
            "/tmp:/tmp".to_string(),
            "--log-opt=max-size=10m".to_string(),
        ];
        assert!(!extra_args_specify_log_driver(&extra));
        assert_eq!(resolve_log_driver_flag(&extra), Some("--log-driver=none"));
    }

    // --- Dockerfile content ----------------------------------------------

    #[test]
    fn dockerfile_has_two_stages() {
        let out = dockerfile_contents("node:20-alpine");
        assert!(out.contains("FROM rust:"));
        assert!(out.contains("AS agent-builder"));
        assert!(out.contains("FROM node:20-alpine"));
    }

    #[test]
    fn dockerfile_references_user_image() {
        let out = dockerfile_contents("python:3.12-slim");
        assert!(out.contains("FROM python:3.12-slim"));
    }

    #[test]
    fn dockerfile_sets_agent_as_entrypoint() {
        let out = dockerfile_contents("alpine:3.20");
        assert!(out.contains(r#"ENTRYPOINT ["/usr/local/bin/mcp-proxy-agent"]"#));
    }

    #[test]
    fn dockerfile_copies_agent_from_builder_stage() {
        let out = dockerfile_contents("alpine:3.20");
        assert!(out.contains("COPY --from=agent-builder"));
        assert!(out.contains("/usr/local/bin/mcp-proxy-agent"));
    }

    // --- Tag computation -------------------------------------------------

    #[test]
    fn image_tag_is_deterministic_for_same_inputs() {
        let tmp = TempDir::new().unwrap();
        let env = HashMap::new();
        let args = vec!["-y".to_string(), "@mcp/srv".to_string()];
        let extra: Vec<String> = vec![];
        let a = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:20-alpine",
            "npx",
            &args,
            &extra,
        ));
        let b = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:20-alpine",
            "npx",
            &args,
            &extra,
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn image_tag_changes_when_args_change() {
        let tmp = TempDir::new().unwrap();
        let env = HashMap::new();
        let extra: Vec<String> = vec![];
        let a = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:20-alpine",
            "npx",
            &["-y".to_string(), "@mcp/a".to_string()],
            &extra,
        ));
        let b = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:20-alpine",
            "npx",
            &["-y".to_string(), "@mcp/b".to_string()],
            &extra,
        ));
        assert_ne!(a, b);
    }

    #[test]
    fn image_tag_changes_when_base_image_changes() {
        let tmp = TempDir::new().unwrap();
        let env = HashMap::new();
        let args: Vec<String> = vec![];
        let extra: Vec<String> = vec![];
        let a = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:20-alpine",
            "npx",
            &args,
            &extra,
        ));
        let b = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:22-alpine",
            "npx",
            &args,
            &extra,
        ));
        assert_ne!(a, b);
    }

    #[test]
    fn image_tag_starts_with_local_prefix() {
        let tmp = TempDir::new().unwrap();
        let env = HashMap::new();
        let args: Vec<String> = vec![];
        let extra: Vec<String> = vec![];
        let tag = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env,
            "node:20-alpine",
            "npx",
            &args,
            &extra,
        ));
        assert!(
            tag.starts_with("mcp-proxy-local/"),
            "tag should be under mcp-proxy-local/ to make cleanup easy; got {tag}"
        );
    }

    #[test]
    fn image_tag_is_independent_of_env_vars_and_extra_args() {
        // env_vars carry secrets — they must NOT affect image identity, because
        // the image is cached across sessions while secrets change frequently.
        // extra_args are runtime docker flags, likewise not part of image identity.
        let tmp = TempDir::new().unwrap();
        let env_empty = HashMap::new();
        let mut env_full = HashMap::new();
        env_full.insert("TOKEN".into(), "secret".into());

        let args: Vec<String> = vec!["run".into()];
        let extra_none: Vec<String> = vec![];
        let extra_net: Vec<String> = vec!["--network=none".into()];

        let a = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env_empty,
            "alpine:3.20",
            "/bin/cat",
            &args,
            &extra_none,
        ));
        let b = compute_image_tag(&sample_cfg(
            tmp.path(),
            &env_full,
            "alpine:3.20",
            "/bin/cat",
            &args,
            &extra_net,
        ));
        assert_eq!(a, b, "env_vars and extra_args must not affect image tag");
    }

    // --- Sanitizer -------------------------------------------------------

    #[test]
    fn sanitize_component_lowercases_and_replaces() {
        assert_eq!(sanitize_component("GitHub_Test!"), "github-test");
        assert_eq!(sanitize_component("---weird---"), "weird");
        assert_eq!(sanitize_component("normal"), "normal");
    }

    #[test]
    fn sanitize_component_never_returns_empty() {
        assert_eq!(sanitize_component(""), "server");
        assert_eq!(sanitize_component("!!!"), "server");
    }

    // --- Build context ---------------------------------------------------

    #[test]
    fn write_build_context_creates_all_files() {
        let tmp = TempDir::new().unwrap();
        let ctx = tmp.path().join("ctx");
        write_build_context(&ctx, "node:20-alpine").unwrap();

        assert!(ctx.join("Dockerfile").is_file());
        assert!(ctx.join("agent-src/Cargo.toml").is_file());
        assert!(ctx.join("agent-src/src/main.rs").is_file());

        let dockerfile = fs::read_to_string(ctx.join("Dockerfile")).unwrap();
        assert!(dockerfile.contains("FROM node:20-alpine"));

        let agent_cargo = fs::read_to_string(ctx.join("agent-src/Cargo.toml")).unwrap();
        assert_eq!(agent_cargo, AGENT_CARGO_TOML);

        let agent_main = fs::read_to_string(ctx.join("agent-src/src/main.rs")).unwrap();
        assert_eq!(agent_main, AGENT_MAIN_RS);
    }

    // --- build_run_argv (pure) -------------------------------------------

    #[test]
    fn build_run_argv_untrusted_default_injects_log_and_network_none() {
        let argv = build_run_argv("tag:abc", &[], false);
        assert_eq!(
            argv,
            vec![
                "run".to_string(),
                "-i".into(),
                "--rm".into(),
                "--log-driver=none".into(),
                "--network=none".into(),
                "tag:abc".into(),
            ]
        );
    }

    #[test]
    fn build_run_argv_trusted_omits_network_but_still_gates_logs() {
        let argv = build_run_argv("tag:abc", &[], true);
        assert_eq!(
            argv,
            vec![
                "run".to_string(),
                "-i".into(),
                "--rm".into(),
                "--log-driver=none".into(),
                "tag:abc".into(),
            ]
        );
    }

    #[test]
    fn build_run_argv_preserves_extra_args_order_after_defaults() {
        // extra_args land after our injected defaults but before the image tag.
        let extra = vec!["-v".to_string(), "/tmp:/tmp".into(), "--memory=256m".into()];
        let argv = build_run_argv("tag:abc", &extra, true);
        assert_eq!(
            argv,
            vec![
                "run".to_string(),
                "-i".into(),
                "--rm".into(),
                "--log-driver=none".into(),
                "-v".into(),
                "/tmp:/tmp".into(),
                "--memory=256m".into(),
                "tag:abc".into(),
            ]
        );
    }

    #[test]
    fn build_run_argv_operator_overrides_win() {
        // Explicit operator flags must suppress our injected defaults.
        let extra = vec![
            "--log-driver=json-file".to_string(),
            "--network=host".into(),
        ];
        let argv = build_run_argv("tag:abc", &extra, false);
        assert_eq!(
            argv,
            vec![
                "run".to_string(),
                "-i".into(),
                "--rm".into(),
                "--log-driver=json-file".into(),
                "--network=host".into(),
                "tag:abc".into(),
            ]
        );
    }

    // --- Fake-docker integration -----------------------------------------
    //
    // These tests plant a shell script that impersonates `docker`, then pass
    // its path to the refactored primitives. CI can run them without a real
    // Docker daemon. Unix-only — the approach relies on `#!/bin/sh` shebangs.

    #[cfg(unix)]
    mod fake_docker {
        use super::super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use tempfile::TempDir;

        /// Drop a minimal shell script at `<dir>/fake-docker.sh` with the given
        /// body and return its path. The script is marked executable so
        /// `Command::new(path)` can launch it directly.
        fn write_fake(dir: &Path, body: &str) -> PathBuf {
            let path = dir.join("fake-docker.sh");
            let full = format!("#!/bin/sh\n{body}\n");
            fs::write(&path, full).unwrap();
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
            path
        }

        #[test]
        fn ensure_available_happy_path() {
            let tmp = TempDir::new().unwrap();
            let bin = write_fake(
                tmp.path(),
                r#"if [ "$1" = "--version" ]; then
  echo "Docker version fake"
  exit 0
fi
exit 99"#,
            );
            ensure_docker_available(bin.as_os_str()).expect("version check should succeed");
        }

        #[test]
        fn ensure_available_reports_nonzero_exit() {
            let tmp = TempDir::new().unwrap();
            let bin = write_fake(tmp.path(), r#"echo "no daemon" 1>&2; exit 1"#);
            let err = ensure_docker_available(bin.as_os_str()).unwrap_err();
            assert!(
                err.contains("no daemon"),
                "stderr from fake docker should surface in the error: {err}"
            );
        }

        #[test]
        fn ensure_available_reports_missing_binary() {
            let missing = Path::new("/definitely/not/a/real/docker/path");
            let err = ensure_docker_available(missing.as_os_str()).unwrap_err();
            assert!(
                err.contains("not installed") || err.contains("not on PATH"),
                "missing binary should produce the install-docker hint: {err}"
            );
        }

        #[test]
        fn image_exists_true_when_fake_exits_zero() {
            let tmp = TempDir::new().unwrap();
            let bin = write_fake(tmp.path(), "exit 0");
            assert!(image_exists(bin.as_os_str(), "whatever:tag").unwrap());
        }

        #[test]
        fn image_exists_false_when_fake_exits_nonzero() {
            let tmp = TempDir::new().unwrap();
            let bin = write_fake(tmp.path(), "exit 1");
            assert!(!image_exists(bin.as_os_str(), "whatever:tag").unwrap());
        }

        /// Fake docker records the argv it sees to a file so the test can
        /// verify flag order end-to-end. This is the closest we can get to a
        /// Docker integration test on CI without a daemon.
        #[test]
        fn docker_build_invokes_bin_with_expected_argv() {
            let tmp = TempDir::new().unwrap();
            let log = tmp.path().join("argv.log");
            let log_str = log.display().to_string();
            let bin = write_fake(
                tmp.path(),
                &format!(
                    r#"printf '%s\n' "$@" > "{log_str}"
exit 0"#
                ),
            );
            let ctx = tmp.path().join("ctx");
            fs::create_dir_all(&ctx).unwrap();
            docker_build(bin.as_os_str(), &ctx, "mcp-proxy-local/srv:abc").expect("build ok");

            let recorded = fs::read_to_string(&log).unwrap();
            let lines: Vec<&str> = recorded.lines().collect();
            assert_eq!(lines[0], "build");
            assert_eq!(lines[1], "-t");
            assert_eq!(lines[2], "mcp-proxy-local/srv:abc");
            assert_eq!(lines[3], ctx.display().to_string());
        }

        #[test]
        fn docker_build_propagates_nonzero_exit() {
            let tmp = TempDir::new().unwrap();
            let bin = write_fake(tmp.path(), r#"echo "build broke" 1>&2; exit 42"#);
            let ctx = tmp.path().join("ctx");
            fs::create_dir_all(&ctx).unwrap();
            let err = docker_build(bin.as_os_str(), &ctx, "x:y").unwrap_err();
            assert!(err.contains("42"), "exit code should be reported: {err}");
        }
    }

    #[test]
    fn write_build_context_is_idempotent() {
        // Calling twice shouldn't leave behind stale state.
        let tmp = TempDir::new().unwrap();
        let ctx = tmp.path().join("ctx");
        write_build_context(&ctx, "node:20-alpine").unwrap();
        // Plant a stale file
        fs::write(ctx.join("stale.txt"), "should vanish").unwrap();
        write_build_context(&ctx, "python:3.12-alpine").unwrap();
        assert!(
            !ctx.join("stale.txt").exists(),
            "rebuild should wipe stale files"
        );
        assert!(fs::read_to_string(ctx.join("Dockerfile"))
            .unwrap()
            .contains("FROM python:3.12-alpine"));
    }
}
