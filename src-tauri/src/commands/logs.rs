use mcp_proxy_common::audit::{read_recent_audit_logs, AuditLogEntry};

#[tauri::command]
pub async fn list_audit_logs(limit: Option<usize>) -> Result<Vec<AuditLogEntry>, String> {
    read_recent_audit_logs(limit.unwrap_or(50))
}
