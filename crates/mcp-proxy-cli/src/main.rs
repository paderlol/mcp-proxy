//! `mcp-proxy` CLI — wraps MCP server invocations with transparent secret injection.
//!
//! Invoked by AI clients (Claude Desktop, Codex, Cursor, etc.) as configured in
//! their respective config files:
//!   { "command": "mcp-proxy", "args": ["run", "<server-id>"] }
//!
//! Reads server + secret metadata from the same JSON store as the Tauri desktop app,
//! resolves secrets from their backends (Keychain / 1Password / EncryptedFile),
//! spawns the real MCP server with env vars injected, and uses inherited stdio
//! so MCP protocol traffic flows transparently.

mod docker;
mod sandbox;

use clap::{Parser, Subcommand};
use mcp_proxy_common::audit::{append_audit_log, AuditLogEntry, AuditStatus};
use mcp_proxy_common::models::{McpServerConfig, RunMode, SecretMeta};
use mcp_proxy_common::secret_resolver::resolve_secret;
use mcp_proxy_common::store::{
    app_data_dir, load_json, save_json, secrets_meta_path, servers_path,
};
use std::collections::HashMap;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "mcp-proxy")]
#[command(
    about = "MCP Proxy — wrap MCP servers with secure secret injection",
    long_about = "A CLI wrapper that resolves secrets from local storage (Keychain on \
                  macOS, encrypted vault on other platforms) or from 1Password, injects \
                  them as environment variables, and transparently proxies stdio to the \
                  real MCP server process."
)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an MCP server with secrets resolved and injected as env vars
    Run {
        /// Server ID as configured in the MCP Proxy desktop app
        server_id: String,
    },
    /// List all configured MCP servers
    List,
}

fn main() {
    // Keep logging off stdout — stdio is reserved for MCP protocol traffic
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run { server_id } => run_server(&server_id),
        Commands::List => list_servers(),
    };

    if let Err(e) = result {
        eprintln!("mcp-proxy: {e}");
        std::process::exit(1);
    }
}

fn run_server(server_id: &str) -> Result<(), String> {
    // 0. When the "Local" secret backend is the encrypted vault (always on
    //    non-macOS; opt-in on macOS via Settings → Local Storage) we must
    //    unlock it before any secret read. AI clients launching us can't
    //    prompt for a password, so we try two sources in order:
    //
    //      1. A session file written by the GUI at unlock time (preferred —
    //         no /proc env-var leak, lifetime bound to GUI session).
    //      2. `MCP_PROXY_MASTER_PASSWORD` env var (fallback for headless
    //         setups or users who haven't launched the GUI this session).
    //
    //    When the backend is Keychain (macOS default) `is_unlocked()` returns
    //    true and this whole block short-circuits.
    {
        use mcp_proxy_common::local_backend;
        if local_backend::vault_exists() && !local_backend::is_unlocked() {
            // Session file fast path.
            match local_backend::unlock_from_session() {
                Ok(true) => { /* loaded from session */ }
                Ok(false) => {
                    // No session on disk — fall back to env var.
                    match std::env::var("MCP_PROXY_MASTER_PASSWORD") {
                        Ok(pw) => local_backend::unlock_vault(&pw)?,
                        Err(_) => {
                            return Err(
                                "Local vault is locked. Unlock the vault in the MCP Proxy \
                                 desktop app, or set MCP_PROXY_MASTER_PASSWORD before invoking \
                                 mcp-proxy."
                                    .to_string(),
                            );
                        }
                    }
                }
                Err(_e) => {
                    // Stale session (password rotated, vault missing, etc.).
                    // Fall through to env var.
                    tracing::warn!("stale session file; falling back to env var");
                    match std::env::var("MCP_PROXY_MASTER_PASSWORD") {
                        Ok(pw) => local_backend::unlock_vault(&pw)?,
                        Err(_) => {
                            return Err(
                                "Local vault session expired. Unlock the vault again in the \
                                 MCP Proxy desktop app, or set MCP_PROXY_MASTER_PASSWORD."
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
    }

    // 1. Load server configs
    let servers: Vec<McpServerConfig> = load_json(servers_path()).ok_or_else(|| {
        format!(
            "No servers.json found at {}. \
             Add a server via the MCP Proxy desktop app first.",
            servers_path().display()
        )
    })?;

    let config = servers
        .iter()
        .find(|s| s.id == server_id)
        .cloned()
        .ok_or_else(|| format!("Server '{server_id}' not found in servers.json"))?;

    if !config.enabled {
        return Err(format!("Server '{server_id}' is disabled"));
    }

    // Trust gate: untrusted servers must be reviewed and marked Trusted in the
    // desktop app before an AI client is allowed to launch them. Enforced here
    // (before secret resolution) so untrusted configs never trigger a secret
    // read or audit entry.
    //
    // Sandbox escape hatch: an untrusted server MAY launch inside a Docker
    // sandbox if the operator has explicitly set a `--network` flag in
    // `extra_args` — that counts as a deliberate, informed run-with-policy
    // choice. The sandbox runtime then enforces `--network=none` as the
    // default for the untrusted tier (see `docker::resolve_network_flag`).
    if !config.trusted {
        let untrusted_sandbox_allowed = matches!(
            &config.run_mode,
            RunMode::DockerSandbox { extra_args, .. }
                if docker::extra_args_specify_network(extra_args)
        );
        if !untrusted_sandbox_allowed {
            return Err(format!(
                "Server '{}' is not trusted. Either mark it as Trusted in the \
                 MCP Proxy desktop app after reviewing it, or run it in Docker \
                 sandbox mode with an explicit `--network` policy in extra_args \
                 (e.g., `--network=none`).",
                config.name
            ));
        }
    }

    // 2. Load secret metadata
    let secret_metas: Vec<SecretMeta> = load_json(secrets_meta_path()).unwrap_or_default();

    // 3. Resolve env vars from secrets (uses tokio for async resolvers)
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let mut env_vars: HashMap<String, String> = HashMap::new();
    for mapping in &config.env_mappings {
        let meta = secret_metas
            .iter()
            .find(|m| m.id == mapping.secret_ref)
            .ok_or_else(|| {
                format!(
                    "Secret '{}' referenced by env var '{}' not found",
                    mapping.secret_ref, mapping.env_var_name
                )
            })?;

        tracing::debug!(
            "Resolving secret '{}' for env var '{}'",
            meta.id,
            mapping.env_var_name
        );

        let resolved = runtime.block_on(resolve_secret(&meta.id, &meta.source));
        let source_name = match &meta.source {
            mcp_proxy_common::models::SecretSource::Local => "Local",
            mcp_proxy_common::models::SecretSource::OnePassword { .. } => "OnePassword",
        };
        let status = match &resolved {
            Ok(_) => AuditStatus::Success,
            Err(err) => AuditStatus::Error(err.clone()),
        };
        if let Err(err) = append_audit_log(&AuditLogEntry {
            timestamp: chrono::Utc::now(),
            server_id: config.id.clone(),
            secret_id: meta.id.clone(),
            source: source_name.to_string(),
            status,
        }) {
            tracing::warn!("failed to append audit log: {err}");
        }
        let value = resolved?;
        env_vars.insert(mapping.env_var_name.clone(), value);
    }

    tracing::info!(
        "Launching server '{}' (command: {}, {} env vars)",
        config.name,
        config.command,
        env_vars.len()
    );

    // Record the first successful launch timestamp. Observational only —
    // failures here are logged but must not block the launch.
    if config.first_launched_at.is_none() {
        if let Err(err) = record_first_launch(&config.id) {
            tracing::warn!("failed to record first_launched_at: {err}");
        }
    }

    // 4. Dispatch based on run mode
    match &config.run_mode {
        RunMode::Local => spawn_local(&config, env_vars),
        RunMode::DockerSandbox { image, extra_args } => {
            let image = image.as_deref().ok_or_else(|| {
                "Docker sandbox requires a base image — edit the server config and set one \
                 (e.g., `node:20-alpine` for npx-based servers)."
                    .to_string()
            })?;
            let build_root = app_data_dir().join("docker-build");
            docker::run_sandbox(docker::SandboxConfig {
                server_id: &config.id,
                image,
                command: &config.command,
                args: &config.args,
                env_vars: &env_vars,
                extra_args,
                trusted: config.trusted,
                build_root: &build_root,
            })
        }
    }
}

/// Re-read `servers.json`, stamp `first_launched_at` on the matching entry if
/// still unset, and write back. Idempotent: returns Ok without writing if the
/// field is already populated (e.g., a concurrent run beat us to it).
fn record_first_launch(server_id: &str) -> Result<(), String> {
    let path = servers_path();
    let mut servers: Vec<McpServerConfig> = load_json(&path)
        .ok_or_else(|| "servers.json vanished between load and write".to_string())?;

    let mut changed = false;
    for s in &mut servers {
        if s.id == server_id && s.first_launched_at.is_none() {
            s.first_launched_at = Some(chrono::Utc::now());
            changed = true;
            break;
        }
    }

    if changed {
        save_json(&path, &servers);
    }
    Ok(())
}

fn spawn_local(config: &McpServerConfig, env_vars: HashMap<String, String>) -> Result<(), String> {
    // Build the child command. On macOS, if the server opts into local
    // sandboxing, wrap the real command in `sandbox-exec -f <profile>`. The
    // `TempProfile` guard lives until after `child.wait()` returns so the
    // profile file is only removed once the child exits.
    #[cfg(target_os = "macos")]
    let (mut cmd, _profile_guard) = build_local_command_macos(config)?;

    #[cfg(not(target_os = "macos"))]
    let mut cmd = {
        if config.sandbox_local {
            tracing::warn!(
                "sandbox_local=true but this platform has no sandbox-exec; \
                 spawning unsandboxed"
            );
        }
        Command::new(&config.command)
    };

    #[cfg(not(target_os = "macos"))]
    cmd.args(&config.args);

    // Inherit stdio: AI client's stdin/stdout IS our stdin/stdout,
    // and the child inherits them directly. MCP protocol traffic flows through
    // without any manual piping on our side.
    let mut child = cmd
        .envs(&env_vars)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {e}", config.command))?;

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for child process: {e}"))?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }

    Ok(())
}

/// macOS-only: build the `Command` used for Local run mode, wrapping the real
/// command in `sandbox-exec -f <profile>` when `sandbox_local` is set.
///
/// Returns the command *and* a `TempProfile` guard the caller must keep alive
/// for the duration of the child process — dropping it removes the `.sb` file.
/// The guard is `Option::None` when the server opted out or when sandboxing
/// couldn't be set up (we fall back to direct spawn rather than blocking).
#[cfg(target_os = "macos")]
fn build_local_command_macos(
    config: &McpServerConfig,
) -> Result<(Command, Option<sandbox::TempProfile>), String> {
    if !config.sandbox_local {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args);
        return Ok((cmd, None));
    }

    // If sandbox-exec isn't on PATH for some reason, warn and fall back rather
    // than refuse to launch. Users who want a hard-fail should use Docker.
    if which_sandbox_exec().is_none() {
        tracing::warn!("sandbox-exec not found on PATH; falling back to direct spawn");
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args);
        return Ok((cmd, None));
    }

    let cache_dir = sandbox::cache_dir_for(&config.id);
    let profile =
        sandbox::write_temp_profile(&config.id, &cache_dir, sandbox::SandboxNetwork::Allowed)
            .map_err(|e| format!("Failed to write sandbox profile: {e}"))?;

    tracing::info!(
        "wrapping '{}' in sandbox-exec (profile: {})",
        config.command,
        profile.path().display()
    );

    let mut cmd = Command::new("sandbox-exec");
    cmd.arg("-f").arg(profile.path());
    cmd.arg(&config.command);
    cmd.args(&config.args);
    Ok((cmd, Some(profile)))
}

#[cfg(target_os = "macos")]
fn which_sandbox_exec() -> Option<std::path::PathBuf> {
    // Fixed location on every supported macOS release; avoid a full PATH
    // scan.
    let p = std::path::PathBuf::from("/usr/bin/sandbox-exec");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn list_servers() -> Result<(), String> {
    let servers: Vec<McpServerConfig> = load_json(servers_path()).unwrap_or_default();

    if servers.is_empty() {
        println!("No MCP servers configured.");
        println!("Add servers via the MCP Proxy desktop app, then they'll appear here.");
        return Ok(());
    }

    println!("Configured MCP servers:\n");
    for s in &servers {
        let status = if s.enabled { "enabled" } else { "disabled" };
        let mode = match &s.run_mode {
            RunMode::Local => "local",
            RunMode::DockerSandbox { .. } => "docker",
        };
        println!("  {}  ({}, {})", s.id, status, mode);
        println!("    name:    {}", s.name);
        println!("    command: {} {}", s.command, s.args.join(" "));
        if !s.env_mappings.is_empty() {
            println!("    env vars:");
            for m in &s.env_mappings {
                println!("      {} → secret:{}", m.env_var_name, m.secret_ref);
            }
        }
        println!();
    }
    Ok(())
}
