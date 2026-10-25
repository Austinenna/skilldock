//! Tauri commands. All skill logic lives in `skilldock_core` (crates/skl);
//! this layer only maps it to serialisable shapes for the frontend.

use crate::error::Result;
use chrono::{DateTime, Local};
use humansize::{format_size, BINARY};
use serde::Serialize;
use skilldock_core as core;
use std::fs;
use std::time::SystemTime;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub path: String,
    pub installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub description: String,
    /// devOnly | released | devAhead | releasedOnly | manifestOnly
    pub state: &'static str,
    pub agents: Vec<String>,
    pub has_dev: bool,
    pub has_released: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub root: String,
    pub root_source: String,
    pub manifest_path: String,
    pub agents: Vec<Agent>,
    pub skills: Vec<Skill>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFile {
    pub name: String,
    pub kind: String,
    pub size: String,
    pub modified: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    pub skill_md: String,
    pub dev_path: Option<String>,
    pub released_path: Option<String>,
    pub files: Vec<SkillFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncLine {
    pub agent: String,
    pub name: String,
    /// created | fixed | removed | conflict
    pub action: &'static str,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub changes: Vec<SyncLine>,
    pub kept: usize,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub git: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// error | warn | info
    pub level: &'static str,
    pub message: String,
}

fn state_id(s: &core::State) -> &'static str {
    match s {
        core::State::DevOnly => "devOnly",
        core::State::Released => "released",
        core::State::DevAhead => "devAhead",
        core::State::ReleasedOnly => "releasedOnly",
        core::State::ManifestOnly => "manifestOnly",
    }
}

fn sync_result(rep: core::SyncReport, warnings: Vec<String>, git: Option<String>) -> SyncResult {
    let mut changes = Vec::new();
    let mut kept = 0;
    for it in rep.items {
        let (action, message) = match it.action {
            core::Action::Kept => {
                kept += 1;
                continue;
            }
            core::Action::Created => ("created", "创建链接".to_string()),
            core::Action::Fixed(old) => ("fixed", format!("修正链接（原指向 {}）", core::tildify(&old))),
            core::Action::Removed => ("removed", "删除多余链接".to_string()),
            core::Action::Conflict(why) => ("conflict", why),
        };
        changes.push(SyncLine { agent: it.agent, name: it.name, action, message });
    }
    SyncResult { changes, kept, notes: rep.notes, warnings, git }
}

fn relative_time(t: SystemTime) -> String {
    let secs = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        86_400..=604_799 => format!("{}d ago", secs / 86_400),
        _ => DateTime::<Local>::from(t).format("%Y-%m-%d").to_string(),
    }
}

#[tauri::command]
pub fn get_overview() -> Result<Overview> {
    let root = core::resolve_root()?;
    let root_source = match &root.source {
        core::RootSource::Env => "SKILLDOCK_HOME".to_string(),
        core::RootSource::Config(p) => core::tildify(p),
        core::RootSource::Default => "default".to_string(),
    };
    let manifest = core::Manifest::load(&root)?;
    let agents = manifest
        .agents()?
        .into_iter()
        .map(|(id, path)| Agent {
            installed: path.exists() || path.parent().map(|p| p.exists()).unwrap_or(false),
            path: core::tildify(&path),
            id,
        })
        .collect();
    let skills = core::status(&root)?
        .into_iter()
        .map(|s| Skill {
            has_dev: root.dev().join(&s.name).is_dir(),
            has_released: root.released().join(&s.name).is_dir(),
            state: state_id(&s.state),
            description: s.description.unwrap_or_default(),
            agents: s.agents,
            id: s.name,
        })
        .collect();
    Ok(Overview {
        root: core::tildify(&root.path),
        root_source,
        manifest_path: core::tildify(&root.manifest_path()),
        agents,
        skills,
    })
}

#[tauri::command]
pub fn get_skill_detail(id: String) -> Result<SkillDetail> {
    core::check_name(&id)?;
    let root = core::resolve_root()?;
    let dev = root.dev().join(&id);
    let rel = root.released().join(&id);
    let src = if dev.is_dir() { &dev } else { &rel };
    let files = core::list_entries(src)
        .into_iter()
        .map(|e| SkillFile {
            kind: if e.is_dir { "directory".into() } else { "file".into() },
            size: if e.is_dir { String::new() } else { format_size(e.size, BINARY) },
            modified: e.modified.map(relative_time).unwrap_or_default(),
            name: e.name,
        })
        .collect();
    Ok(SkillDetail {
        skill_md: fs::read_to_string(src.join("SKILL.md")).unwrap_or_default(),
        dev_path: dev.is_dir().then(|| core::tildify(&dev)),
        released_path: rel.is_dir().then(|| core::tildify(&rel)),
        files,
    })
}

#[tauri::command]
pub fn new_skill(name: String) -> Result<String> {
    let dir = core::new_skill(&core::resolve_root()?, &name)?;
    Ok(core::tildify(&dir))
}

/// `agents: None` keeps the agents already in the manifest.
#[tauri::command]
pub fn publish_skill(name: String, agents: Option<Vec<String>>) -> Result<SyncResult> {
    let rep = core::publish(&core::resolve_root()?, &name, agents)?;
    Ok(sync_result(rep.sync, rep.warnings, rep.git))
}

#[tauri::command]
pub fn set_skill_agents(name: String, agents: Vec<String>) -> Result<SyncResult> {
    let rep = core::set_agents(&core::resolve_root()?, &name, &agents)?;
    Ok(sync_result(rep, Vec::new(), None))
}

#[tauri::command]
pub fn unpublish_skill(name: String) -> Result<SyncResult> {
    let rep = core::unpublish(&core::resolve_root()?, &name)?;
    Ok(sync_result(rep, Vec::new(), None))
}

/// Unpublishes if needed and moves dev/<name> into <root>/.trash/.
#[tauri::command]
pub fn delete_skill(name: String) -> Result<Option<String>> {
    let rep = core::delete(&core::resolve_root()?, &name)?;
    Ok(rep.trashed.map(|p| core::tildify(&p)))
}

#[tauri::command]
pub fn run_sync(dry_run: bool) -> Result<SyncResult> {
    let rep = core::sync(&core::resolve_root()?, dry_run)?;
    Ok(sync_result(rep, Vec::new(), None))
}

#[tauri::command]
pub fn run_doctor() -> Result<Vec<Finding>> {
    Ok(core::doctor(&core::resolve_root()?)?
        .into_iter()
        .map(|f| Finding {
            level: match f.level {
                core::Level::Error => "error",
                core::Level::Warn => "warn",
                core::Level::Info => "info",
            },
            message: f.msg,
        })
        .collect())
}
