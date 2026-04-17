use crate::store::AppState;
use mcp_proxy_common::models::{SecretMeta, SecretSource};
use mcp_proxy_common::secret_resolver::{delete_secret_local, resolve_secret, store_secret_local};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretEntry {
    pub id: String,
    pub label: String,
    pub source: SecretSource,
}

#[tauri::command]
pub async fn list_secrets(state: State<'_, AppState>) -> Result<Vec<SecretEntry>, String> {
    let metas = state.secrets_meta.lock().map_err(|e| e.to_string())?;
    Ok(metas
        .iter()
        .map(|m| SecretEntry {
            id: m.id.clone(),
            label: m.label.clone(),
            source: m.source.clone(),
        })
        .collect())
}

#[tauri::command]
pub async fn get_secret(id: String, source: SecretSource) -> Result<String, String> {
    resolve_secret(&id, &source).await
}

#[tauri::command]
pub async fn set_secret(
    id: String,
    label: String,
    // `None` means the caller is editing metadata only (e.g., label). The
    // Keychain entry / vault is left untouched. Useful when the UI lets the
    // user leave the "value" field blank to keep the previously stored secret.
    value: Option<String>,
    source: SecretSource,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(v) = &value {
        match &source {
            SecretSource::Local => {
                store_secret_local(&id, v)?;
            }
            SecretSource::OnePassword { .. } => {
                // Reference-only backend — `value` is ignored even if present.
            }
        }
    }

    let mut metas = state.secrets_meta.lock().map_err(|e| e.to_string())?;
    if let Some(existing) = metas.iter_mut().find(|m| m.id == id) {
        existing.label = label;
        existing.source = source;
        existing.updated_at = chrono::Utc::now();
    } else {
        metas.push(SecretMeta::new(id, label, source));
    }
    state.save_secrets_meta(&metas);
    Ok(())
}

#[tauri::command]
pub async fn delete_secret(
    id: String,
    source: SecretSource,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if matches!(source, SecretSource::Local) {
        delete_secret_local(&id)?;
    }

    let mut metas = state.secrets_meta.lock().map_err(|e| e.to_string())?;
    metas.retain(|m| m.id != id);
    state.save_secrets_meta(&metas);
    Ok(())
}
