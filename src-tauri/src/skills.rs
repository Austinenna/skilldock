use crate::error::{AppError, Result};
use crate::platforms::{central_dir, expand_path, PLATFORMS};
use chrono::{DateTime, Local};
use humansize::{format_size, BINARY};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SkillMetadata {
    title: String,
    tagline: String,
    version: String,
    tags: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawFrontmatter {
    name: Option<Value>,
    description: Option<Value>,
    version: Option<Value>,
    tags: Option<Value>,
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

pub fn central_skill_path(id: &str) -> Result<PathBuf> {
    validate_skill_id(id)?;
    Ok(central_dir().ok_or(AppError::NoHomeDir)?.join(id))
}

fn canonical_central_dir() -> Result<PathBuf> {
    ensure_central()?;
    Ok(central_dir().ok_or(AppError::NoHomeDir)?.canonicalize()?)
}

pub fn existing_central_skill_paths(id: &str) -> Result<(PathBuf, PathBuf)> {
    let path = central_skill_path(id)?;
    if !path.exists() {
        return Err(AppError::SkillNotFound(id.to_string()));
    }

    let canonical_central = canonical_central_dir()?;
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&canonical_central) {
        return Err(AppError::Conflict(format!(
            "skill path for '{}' resolves outside central",
            id
        )));
    }

    Ok((path, canonical_path))
}

fn compact_text(value: &str) -> Option<String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        None
    } else {
        Some(compact)
    }
}

fn value_to_string(value: Value) -> Option<String> {
    match value {
        Value::String(value) => compact_text(&value),
        Value::Number(value) => compact_text(&value.to_string()),
        Value::Bool(value) => compact_text(&value.to_string()),
        _ => None,
    }
}

fn value_to_tags(value: Value) -> Vec<String> {
    match value {
        Value::Sequence(values) => values.into_iter().filter_map(value_to_string).collect(),
        Value::String(value) => {
            let parts: Vec<&str> = if value.contains(',') {
                value.split(',').collect()
            } else {
                value.split_whitespace().collect()
            };
            parts.into_iter().filter_map(compact_text).collect()
        }
        _ => Vec::new(),
    }
}

fn split_frontmatter(content: &str) -> (Option<String>, String) {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, content.to_string());
    }

    let mut yaml = Vec::new();
    let mut body = Vec::new();
    let mut found_end = false;

    for line in lines {
        if !found_end && line.trim() == "---" {
            found_end = true;
            continue;
        }

        if found_end {
            body.push(line);
        } else {
            yaml.push(line);
        }
    }

    if found_end {
        (Some(yaml.join("\n")), body.join("\n"))
    } else {
        (None, content.to_string())
    }
}

fn body_fallback(content: &str) -> String {
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t == "---" {
            continue;
        }
        if t.starts_with("name:")
            || t.starts_with("description:")
            || t.starts_with("version:")
            || t.starts_with("tags:")
        {
            continue;
        }
        return t.to_string();
    }
    String::new()
}

fn parse_skill_metadata(content: &str, fallback_id: &str) -> SkillMetadata {
    let (frontmatter, body) = split_frontmatter(content);
    let parsed = frontmatter
        .as_deref()
        .and_then(|yaml| serde_yaml::from_str::<RawFrontmatter>(yaml).ok());

    let title = parsed
        .as_ref()
        .and_then(|fm| fm.name.clone())
        .and_then(value_to_string)
        .unwrap_or_else(|| fallback_id.to_string());
    let tagline = parsed
        .as_ref()
        .and_then(|fm| fm.description.clone())
        .and_then(value_to_string)
        .unwrap_or_else(|| body_fallback(&body));
    let version = parsed
        .as_ref()
        .and_then(|fm| fm.version.clone())
        .and_then(value_to_string)
        .unwrap_or_default();
    let tags = parsed
        .and_then(|fm| fm.tags)
        .map(value_to_tags)
        .unwrap_or_default();

    SkillMetadata {
        title,
        tagline,
        version,
        tags,
    }
}

fn read_skill_metadata(skill_dir: &Path, fallback_id: &str) -> SkillMetadata {
    let skill_md = skill_dir.join("SKILL.md");
    let Ok(content) = fs::read_to_string(&skill_md) else {
        return SkillMetadata {
            title: fallback_id.to_string(),
            ..SkillMetadata::default()
        };
    };

    parse_skill_metadata(&content, fallback_id)
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

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn comparable_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| normalize_path(path))
}

fn resolve_link_path(link: PathBuf, target: &Path) -> PathBuf {
    if link.is_absolute() {
        link
    } else {
        target.parent().map(|d| d.join(&link)).unwrap_or(link)
    }
}

pub fn link_points_to_path(
    link: PathBuf,
    target: &Path,
    source: &Path,
    canonical_source: &Path,
) -> bool {
    let resolved = resolve_link_path(link, target);
    comparable_path(&resolved) == canonical_source
        || normalize_path(&resolved) == normalize_path(source)
}

/// Which platforms currently have a symlink for this skill pointing back to central.
/// "central" is always included if the skill exists in central. Entries that block
/// route creation are reported separately so the UI can explain why a switch is off.
pub fn compute_routes(skill_id: &str) -> (Vec<String>, Vec<RouteConflict>) {
    let mut routes = vec!["central".to_string()];
    let mut conflicts = Vec::new();
    let Ok((source, canonical_source)) = existing_central_skill_paths(skill_id) else {
        return (routes, conflicts);
    };

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
        if link_points_to_path(link.clone(), &target, &source, &canonical_source) {
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
        let metadata = read_skill_metadata(&path, &name);
        skills.push(Skill {
            id: name.clone(),
            title: metadata.title,
            tagline: metadata.tagline,
            version: metadata.version,
            size: format_size(total_size, BINARY),
            files,
            updated,
            tags: metadata.tags,
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
    let dir = central_skill_path(&id)?;
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
    let (dir, canonical_dir) = existing_central_skill_paths(&id)?;
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
            if link_points_to_path(link, &target, &dir, &canonical_dir) {
                let _ = fs::remove_file(&target);
            }
        }
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{link_points_to_path, parse_skill_metadata, validate_skill_id, SkillMetadata};
    use std::path::PathBuf;

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

    #[test]
    fn compares_links_with_missing_normalized_components() {
        let source = PathBuf::from("/tmp/skillloom-central/example");
        let canonical_source = source.clone();
        let target = PathBuf::from("/tmp/skillloom-platform/example");
        let link = PathBuf::from("/tmp/skillloom-central/missing/../example");

        assert!(link_points_to_path(
            link,
            &target,
            &source,
            &canonical_source
        ));
    }

    #[test]
    fn parses_normal_frontmatter_metadata() {
        let metadata = parse_skill_metadata(
            "---\nname: CSV Cleaner\ndescription: Clean CSV files\nversion: 1.2.3\ntags:\n  - csv\n  - cleanup\n---\n\n# CSV Cleaner\n",
            "csv-cleaner",
        );

        assert_eq!(
            metadata,
            SkillMetadata {
                title: "CSV Cleaner".into(),
                tagline: "Clean CSV files".into(),
                version: "1.2.3".into(),
                tags: vec!["csv".into(), "cleanup".into()],
            }
        );
    }

    #[test]
    fn parses_quoted_description_and_string_tags() {
        let metadata = parse_skill_metadata(
            "---\nname: quoted-skill\ndescription: \"Handles quoted: values\"\ntags: \"quotes, yaml\"\n---\n",
            "fallback",
        );

        assert_eq!(metadata.title, "quoted-skill");
        assert_eq!(metadata.tagline, "Handles quoted: values");
        assert_eq!(metadata.tags, vec!["quotes", "yaml"]);
    }

    #[test]
    fn falls_back_to_body_when_description_is_missing() {
        let metadata = parse_skill_metadata(
            "---\nname: body-skill\n---\n\n# Body Skill\n\nUse the first body sentence.",
            "fallback",
        );

        assert_eq!(metadata.title, "body-skill");
        assert_eq!(metadata.tagline, "Use the first body sentence.");
        assert_eq!(metadata.version, "");
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn falls_back_when_fields_are_missing() {
        let metadata = parse_skill_metadata("Plain body fallback.", "missing-fields");

        assert_eq!(metadata.title, "missing-fields");
        assert_eq!(metadata.tagline, "Plain body fallback.");
        assert_eq!(metadata.version, "");
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn tolerates_malformed_frontmatter() {
        let metadata = parse_skill_metadata(
            "---\nname: [broken\n---\n\n# Broken\n\nStill readable.",
            "broken-skill",
        );

        assert_eq!(metadata.title, "broken-skill");
        assert_eq!(metadata.tagline, "Still readable.");
        assert_eq!(metadata.version, "");
        assert!(metadata.tags.is_empty());
    }
}
