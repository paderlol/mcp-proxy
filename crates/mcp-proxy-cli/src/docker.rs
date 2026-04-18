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

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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
pub fn run_sandbox(cfg: SandboxConfig) -> Result<(), String> {
    ensure_docker_available()?;

    let tag = compute_image_tag(&cfg);

    if !image_exists(&tag)? {
        let ctx_dir = cfg.build_root.join(sanitize_component(cfg.server_id));
        write_build_context(&ctx_dir, cfg.image)?;
        docker_build(&ctx_dir, &tag)?;
    }

    let payload = SecretPayload {
        env_vars: cfg.env_vars,
        command: cfg.command,
        args: cfg.args,
    };

    docker_run_with_stdin_payload(&tag, cfg.extra_args, cfg.trusted, &payload)
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
// docker CLI detection
// ---------------------------------------------------------------------------

fn ensure_docker_available() -> Result<(), String> {
    match Command::new("docker").arg("--version").output() {
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

fn image_exists(tag: &str) -> Result<bool, String> {
    let output = Command::new("docker")
        .args(["image", "inspect", tag])
        .output()
        .map_err(|e| format!("Failed to invoke `docker image inspect`: {e}"))?;
    Ok(output.status.success())
}

fn docker_build(ctx_dir: &Path, tag: &str) -> Result<(), String> {
    eprintln!("mcp-proxy: building sandbox image {tag} (first build may take ~2 min)");
    let status = Command::new("docker")
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

fn docker_run_with_stdin_payload(
    tag: &str,
    extra_args: &[String],
    trusted: bool,
    payload: &SecretPayload,
) -> Result<(), String> {
    let mut cmd = Command::new("docker");
    cmd.args(["run", "-i", "--rm"]);
    if let Some(flag) = resolve_network_flag(trusted, extra_args) {
        cmd.arg(flag);
    }
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.arg(tag);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to invoke `docker run`: {e}"))?;

    // Write the secret payload as the first line. The agent reads exactly one
    // line, so we include a trailing newline and then let the remaining stdio
    // flow from the AI client's stdin into the container.
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
    // MCP traffic. Pump it into the container in a dedicated thread so we can
    // wait on the child in the main thread.
    let pump = std::thread::spawn(move || {
        let mut host_stdin = std::io::stdin();
        let _ = std::io::copy(&mut host_stdin, &mut child_stdin);
        // Dropping child_stdin closes the pipe so the container can see EOF.
    });

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait on container: {e}"))?;

    // The pump thread will finish when host stdin hits EOF or the child exits.
    // Best-effort join; don't block forever on a stuck reader.
    let _ = pump.join();

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
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
