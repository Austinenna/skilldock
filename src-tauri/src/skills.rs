use crate::error::{AppError, Result};
use crate::platforms::{central_dir, expand_path, PLATFORMS};
use chrono::{DateTime, Local};
use humansize::{format_size, BINARY};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteConflict {
    pub platform_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub title: String,
    pub tagline: String,
    pub version: String,
    pub size: String,
    pub files: usize,
    pub updated: String,
    pub tags: Vec<String>,
    pub routes: Vec<String>,
    pub route_conflicts: Vec<RouteConflict>,
}

fn ensure_central() -> Result<()> {
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    fs::create_dir_all(&central)?;
    Ok(())
}

fn is_valid_skill_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && id
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

pub fn validate_skill_id(id: &str) -> Result<()> {
    if is_valid_skill_id(id) {
        Ok(())
    } else {
        Err(AppError::InvalidName(id.to_string()))
    }
}

/// Pull `description:` from SKILL.md frontmatter, or fall back to the first
/// non-empty / non-heading line of the body. Empty string if nothing usable.
fn read_tagline(skill_dir: &Path) -> String {
    let skill_md = skill_dir.join("SKILL.md");
    let Ok(content) = fs::read_to_string(&skill_md) else {
        return String::new();
    };

    let mut lines = content.lines();
    let first = lines.next().unwrap_or("").trim();
    if first == "---" {
        for line in lines.by_ref() {
            let t = line.trim();
            if t == "---" {
                break;
            }
            if let Some(rest) = t.strip_prefix("description:") {
                return rest
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
            }
        }
    }
    // Fallback: first non-empty, non-heading line in the rest
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t == "---" {
            continue;
        }
        if t.starts_with("name:") || t.starts_with("description:") {
            continue;
        }
        return t.to_string();
    }
    String::new()
}

fn dir_stats(dir: &Path) -> (usize, u64) {
    let mut count = 0;
    let mut total = 0;
    let Ok(entries) = fs::read_dir(dir) else {
        return (0, 0);
    };
    for e in entries.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_file() {
            count += 1;
            total += meta.len();
        }
    }
    (count, total)
}

fn relative_time(modified: SystemTime) -> String {
    let dt: DateTime<Local> = modified.into();
    let now = Local::now();
    let delta = now.signed_duration_since(dt);
    let secs = delta.num_seconds().max(0);
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{} minutes ago", secs / 60)
    } else if secs < 86_400 {
        format!("{} hours ago", secs / 3600)
    } else if secs < 86_400 * 7 {
        format!("{} days ago", secs / 86_400)
    } else if secs < 86_400 * 30 {
        format!("{} weeks ago", secs / 86_400 / 7)
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}

fn resolve_link_path(link: std::path::PathBuf, target: &Path) -> std::path::PathBuf {
    if link.is_absolute() {
        link
    } else {
        target.parent().map(|d| d.join(&link)).unwrap_or(link)
    }
}

/// Which platforms currently have a symlink for this skill pointing back to central.
/// "central" is always included if the skill exists in central. Entries that block
/// route creation are reported separately so the UI can explain why a switch is off.
pub fn compute_routes(skill_id: &str) -> (Vec<String>, Vec<RouteConflict>) {
    let mut routes = vec!["central".to_string()];
    let mut conflicts = Vec::new();
    let Some(central) = central_dir() else {
        return (routes, conflicts);
    };
    let source = central.join(skill_id);
    let canonical_source = source.canonicalize().unwrap_or_else(|_| source.clone());

    for p in PLATFORMS.iter().filter(|p| !p.is_hub) {
        let Some(platform_root) = expand_path(&p.path) else {
            continue;
        };
        let target = platform_root.join(skill_id);
        let Ok(meta) = fs::symlink_metadata(&target) else {
            continue;
        };

        if !meta.file_type().is_symlink() {
            conflicts.push(RouteConflict {
                platform_id: p.id.clone(),
                message: format!(
                    "{}{} already exists as a real file or directory.",
                    p.path, skill_id
                ),
            });
            continue;
        }

        let Ok(link) = fs::read_link(&target) else {
            conflicts.push(RouteConflict {
                platform_id: p.id.clone(),
                message: format!(
                    "{}{} is a symlink, but SkillLoom could not read its target.",
                    p.path, skill_id
                ),
            });
            continue;
        };
        let resolved = resolve_link_path(link.clone(), &target);
        let canonical_resolved = resolved.canonicalize().unwrap_or(resolved);
        if canonical_resolved == canonical_source {
            routes.push(p.id.clone());
        } else {
            conflicts.push(RouteConflict {
                platform_id: p.id.clone(),
                message: format!(
                    "{}{} is already linked to {}.",
                    p.path,
                    skill_id,
                    link.display()
                ),
            });
        }
    }
    (routes, conflicts)
}

#[tauri::command]
pub fn scan_skills() -> Result<Vec<Skill>> {
    ensure_central()?;
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    let mut skills: Vec<Skill> = Vec::new();

    for entry in fs::read_dir(&central)? {
        let entry = entry?;
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if !meta.is_dir() || validate_skill_id(&name).is_err() {
            continue;
        }
        let path = entry.path();
        let (files, total_size) = dir_stats(&path);
        let updated = meta.modified().ok().map(relative_time).unwrap_or_default();
        let (routes, route_conflicts) = compute_routes(&name);
        skills.push(Skill {
            id: name.clone(),
            title: name.clone(),
            tagline: read_tagline(&path),
            version: String::new(),
            size: format_size(total_size, BINARY),
            files,
            updated,
            tags: Vec::new(),
            routes,
            route_conflicts,
        });
    }

    skills.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(skills)
}

#[tauri::command]
pub fn import_skill(name: String, tagline: String) -> Result<Skill> {
    let id = name.trim().to_lowercase().replace(' ', "-");
    validate_skill_id(&id).map_err(|_| AppError::InvalidName(name))?;
    ensure_central()?;
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    let dir = central.join(&id);
    if dir.exists() {
        return Err(AppError::Conflict(format!(
            "'{}' already exists in central",
            id
        )));
    }
    fs::create_dir_all(&dir)?;

    let desc_for_md = tagline.replace('"', "'");
    let body_desc = if tagline.is_empty() {
        "New skill — fill in what it does."
    } else {
        &tagline
    };
    let skill_md =
        format!("---\nname: {id}\ndescription: \"{desc_for_md}\"\n---\n\n# {id}\n\n{body_desc}\n",);
    fs::write(dir.join("SKILL.md"), skill_md)?;

    let (files, total_size) = dir_stats(&dir);
    Ok(Skill {
        id: id.clone(),
        title: id,
        tagline,
        version: String::new(),
        size: format_size(total_size, BINARY),
        files,
        updated: "just now".into(),
        tags: Vec::new(),
        routes: vec!["central".to_string()],
        route_conflicts: Vec::new(),
    })
}

#[tauri::command]
pub fn delete_skill(id: String) -> Result<()> {
    validate_skill_id(&id)?;
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    let dir = central.join(&id);
    if !dir.exists() {
        return Err(AppError::SkillNotFound(id));
    }
    // Clean up every platform symlink that points to this central skill, but
    // refuse to touch anything that's not a symlink to *our* central path.
    for p in PLATFORMS.iter().filter(|p| !p.is_hub) {
        let Some(platform_root) = expand_path(&p.path) else {
            continue;
        };
        let target = platform_root.join(&id);
        let Ok(meta) = fs::symlink_metadata(&target) else {
            continue;
        };
        if !meta.file_type().is_symlink() {
            continue;
        }
        if let Ok(link) = fs::read_link(&target) {
            let resolved = if link.is_absolute() {
                link
            } else {
                target.parent().map(|d| d.join(&link)).unwrap_or_default()
            };
            if resolved == dir {
                let _ = fs::remove_file(&target);
            }
        }
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_skill_id;

    #[test]
    fn validates_simple_skill_ids() {
        for id in ["csv-cleaner", "skill_loom", "skill.v1", "a0", "x"] {
            assert!(validate_skill_id(id).is_ok(), "{id} should be valid");
        }
    }

    #[test]
    fn rejects_empty_and_dot_paths() {
        for id in ["", ".", ".."] {
            assert!(validate_skill_id(id).is_err(), "{id:?} should be invalid");
        }
    }

    #[test]
    fn rejects_path_traversal_and_separators() {
        for id in ["../secret", "nested/skill", "nested\\skill", "skill/.."] {
            assert!(validate_skill_id(id).is_err(), "{id} should be invalid");
        }
    }

    #[test]
    fn rejects_non_simple_directory_names() {
        for id in [
            "Skill",
            "skill name",
            "skill:name",
            "skill\0name",
            "skill@name",
        ] {
            assert!(validate_skill_id(id).is_err(), "{id:?} should be invalid");
        }
    }
}
