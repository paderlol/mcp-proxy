//! Tauri commands for the Activity page — read-only access to `invocations.db`.

use chrono::{Duration, Utc};
use mcp_proxy_common::invocation_log::{self, SessionRow, ToolCallRow};

#[tauri::command]
pub async fn list_invocation_sessions(
    server_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SessionRow>, String> {
    invocation_log::list_sessions(server_id.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub async fn list_invocation_tool_calls(
    session_id: i64,
    limit: Option<usize>,
) -> Result<Vec<ToolCallRow>, String> {
    invocation_log::list_tool_calls(session_id, limit.unwrap_or(500))
}

#[tauri::command]
pub async fn invocation_counts_by_tool(
    server_id: String,
    since_days: Option<i64>,
) -> Result<Vec<(String, i64)>, String> {
    let since = Utc::now() - Duration::days(since_days.unwrap_or(7));
    invocation_log::tool_call_counts(&server_id, since)
}
