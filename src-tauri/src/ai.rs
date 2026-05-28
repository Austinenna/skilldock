use crate::error::Result;
use keyring::{Entry, Error as KeyringError};
use serde::Serialize;

const KEYRING_SERVICE: &str = "com.skillloom.desktop";
const API_KEY_ACCOUNT: &str = "ai_api_key";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub configured: bool,
}

fn api_key_entry() -> Result<Entry> {
    Ok(Entry::new(KEYRING_SERVICE, API_KEY_ACCOUNT)?)
}

pub fn read_api_key() -> Result<Option<String>> {
    match api_key_entry()?.get_password() {
        Ok(key) if !key.trim().is_empty() => Ok(Some(key)),
        Ok(_) => Ok(None),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub fn get_api_key_status() -> Result<ApiKeyStatus> {
    Ok(ApiKeyStatus {
        configured: read_api_key()?.is_some(),
    })
}

#[tauri::command]
pub fn set_api_key(key: String) -> Result<ApiKeyStatus> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        clear_api_key()?;
    } else {
        api_key_entry()?.set_password(trimmed)?;
    }
    get_api_key_status()
}

#[tauri::command]
pub fn clear_api_key() -> Result<ApiKeyStatus> {
    match api_key_entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => get_api_key_status(),
        Err(error) => Err(error.into()),
    }
}
