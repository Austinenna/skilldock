use once_cell::sync::Lazy;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub short: String,
    pub path: String,
    pub group: String,
    pub is_hub: bool,
}

fn pl(id: &str, name: &str, short: &str, path: &str, group: &str, is_hub: bool) -> Platform {
    Platform {
        id: id.into(), name: name.into(), short: short.into(),
        path: path.into(), group: group.into(), is_hub,
    }
}

pub static PLATFORMS: Lazy<Vec<Platform>> = Lazy::new(|| vec![
    pl("central",   "Central Skills",      "Central",   "~/.skillloom/skills/",                       "Core",    true),
    pl("claude",    "Claude Code",         "Claude",    "~/.claude/skills/",                          "Coding",  false),
    pl("codex",     "Codex CLI",           "Codex",     "~/.agents/skills/",                          "Coding",  false),
    pl("openclaw",  "OpenClaw 开爪",       "OpenClaw",  "~/.openclaw/skills/",                        "Lobster", false),
    pl("cursor",    "Cursor",              "Cursor",    "~/.cursor/skills/",                          "Coding",  false),
    pl("gemini",    "Gemini CLI",          "Gemini",    "~/.gemini/skills/",                          "Coding",  false),
    pl("copilot",   "Copilot",             "Copilot",   "~/.copilot/skills/",                         "Coding",  false),
    pl("windsurf",  "Windsurf",            "Windsurf",  "~/.windsurf/skills/",                        "Coding",  false),
    pl("aider",     "Aider",               "Aider",     "~/.aider/skills/",                           "Coding",  false),
    pl("qclaw",     "QClaw 千爪",          "QClaw",     "~/.qclaw/skills/",                           "Lobster", false),
    pl("easyclaw",  "EasyClaw 简爪",       "EasyClaw",  "~/.easyclaw/skills/",                        "Lobster", false),
    pl("workbuddy", "WorkBuddy 打工搭子",  "WorkBuddy", "~/.workbuddy/skills-marketplace/skills/",    "Lobster", false),
]);

pub fn expand_path(path_with_tilde: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if let Some(rest) = path_with_tilde.strip_prefix("~/") {
        Some(home.join(rest))
    } else if path_with_tilde == "~" {
        Some(home)
    } else {
        Some(PathBuf::from(path_with_tilde))
    }
}

pub fn platform_by_id(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

pub fn platform_dir(platform_id: &str) -> Option<PathBuf> {
    platform_by_id(platform_id).and_then(|p| expand_path(&p.path))
}

pub fn central_dir() -> Option<PathBuf> {
    platform_dir("central")
}

#[tauri::command]
pub fn list_platforms() -> Vec<Platform> {
    PLATFORMS.clone()
}
