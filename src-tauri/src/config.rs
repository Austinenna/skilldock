use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub palette: String,
    pub density: String,
    pub view: String,
    pub hidden_platforms: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            palette: "cool".into(),
            density: "comfortable".into(),
            view: "list".into(),
            hidden_platforms: ["cursor", "gemini", "copilot", "windsurf", "aider",
                               "qclaw", "easyclaw", "workbuddy"]
                .iter().map(|s| s.to_string()).collect(),
        }
    }
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_data_dir().map_err(|_| AppError::NoHomeDir)?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("config.json"))
}

fn read_or_default(app: &tauri::AppHandle) -> Result<Config> {
    let path = config_path(app)?;
    if !path.exists() { return Ok(Config::default()); }
    let content = fs::read_to_string(&path)?;
    // If the file is corrupt or stale-schema, fall back to defaults instead of crashing.
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> Result<Config> {
    read_or_default(&app)
}

#[tauri::command]
pub fn update_config(app: tauri::AppHandle, patch: serde_json::Value) -> Result<Config> {
    let current = read_or_default(&app)?;
    let mut merged = serde_json::to_value(&current)?;
    if let (Some(obj), Some(p)) = (merged.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            obj.insert(k.clone(), v.clone());
        }
    }
    let next: Config = serde_json::from_value(merged)?;
    fs::write(config_path(&app)?, serde_json::to_string_pretty(&next)?)?;
    Ok(next)
}
