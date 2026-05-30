use crate::error::{AppError, Result};
use crate::platforms::{central_dir, expand_path, PLATFORMS};
use crate::skills::{link_points_to_path, validate_skill_id};
use chrono::Local;
use serde::Serialize;
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InitAction {
    MigrateToCentral,
    LinkExistingCentral,
    LinkPlannedCentral,
    ResolveConflictSource,
    AlreadyRouted,
    SkipConflict,
    SkipInvalid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitPreviewItem {
    pub key: String,
    pub id: String,
    pub title: String,
    pub platform_id: String,
    pub platform_name: String,
    pub platform_path: String,
    pub source_path: String,
    pub content_path: String,
    pub source_is_symlink: bool,
    pub target_id: String,
    pub target_path: String,
    pub action: InitAction,
    pub selected: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitPreviewSummary {
    pub migratable: usize,
    pub already_routed: usize,
    pub conflicts: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitPreview {
    pub central_path: String,
    pub items: Vec<InitPreviewItem>,
    pub summary: InitPreviewSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitRunItem {
    pub key: String,
    pub id: String,
    pub platform_id: String,
    pub action: InitAction,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitResult {
    pub backup_root: String,
    pub completed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub items: Vec<InitRunItem>,
}

#[derive(Debug, Clone)]
struct PlatformRoot {
    id: String,
    name: String,
    path_label: String,
    root: PathBuf,
}

#[derive(Debug, Clone)]
struct Candidate {
    key: String,
    id: String,
    title: String,
    fingerprint: String,
    platform: PlatformRoot,
    source: PathBuf,
    content_source: PathBuf,
    source_is_symlink: bool,
}

fn item_key(platform_id: &str, id: &str) -> String {
    format!("{platform_id}::{id}")
}

fn selectable(action: &InitAction) -> bool {
    matches!(
        action,
        InitAction::MigrateToCentral
            | InitAction::LinkExistingCentral
            | InitAction::LinkPlannedCentral
            | InitAction::ResolveConflictSource
    )
}

fn summarize(items: &[InitPreviewItem]) -> InitPreviewSummary {
    let mut summary = InitPreviewSummary::default();
    for item in items {
        match item.action {
            InitAction::MigrateToCentral
            | InitAction::LinkExistingCentral
            | InitAction::LinkPlannedCentral => summary.migratable += 1,
            InitAction::ResolveConflictSource => summary.conflicts += 1,
            InitAction::AlreadyRouted => summary.already_routed += 1,
            InitAction::SkipConflict => summary.conflicts += 1,
            InitAction::SkipInvalid => summary.skipped += 1,
        }
    }
    summary
}

fn extract_title_from_skill_md(path: &Path, fallback_id: &str) -> String {
    let Ok(content) = fs::read_to_string(path.join("SKILL.md")) else {
        return fallback_id.to_string();
    };
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fallback_id.to_string();
    }

    let mut yaml = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        yaml.push(line);
    }

    let Ok(value) = serde_yaml::from_str::<serde_yaml::Mapping>(&yaml.join("\n")) else {
        return fallback_id.to_string();
    };
    value
        .get(Value::String("name".into()))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback_id)
        .to_string()
}

fn push_path_marker(hasher: &mut Sha256, kind: &str, relative: &Path) {
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(relative.to_string_lossy().as_bytes());
    hasher.update([0]);
}

fn hash_dir_into(hasher: &mut Sha256, root: &Path, dir: &Path) -> Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        let relative = path.strip_prefix(root).unwrap_or(&path);

        if meta.is_dir() {
            push_path_marker(hasher, "dir", relative);
            hash_dir_into(hasher, root, &path)?;
        } else if meta.is_file() {
            push_path_marker(hasher, "file", relative);
            hasher.update(meta.len().to_le_bytes());
            hasher.update(fs::read(&path)?);
        } else if meta.file_type().is_symlink() {
            push_path_marker(hasher, "symlink", relative);
            hasher.update(fs::read_link(&path)?.to_string_lossy().as_bytes());
        }
    }

    Ok(())
}

fn dir_fingerprint(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    hash_dir_into(&mut hasher, path, path)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn copy_dir_all(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir(dest)?;
    let mut entries = fs::read_dir(source)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let src = entry.path();
        let dst = dest.join(entry.file_name());
        let meta = fs::symlink_metadata(&src)?;
        if meta.is_dir() {
            copy_dir_all(&src, &dst)?;
        } else if meta.is_file() {
            fs::copy(&src, &dst)?;
        } else if meta.file_type().is_symlink() {
            symlink(fs::read_link(&src)?, &dst)?;
        }
    }

    Ok(())
}

fn move_dir_to_backup(source: &Path, backup: &Path) -> Result<()> {
    match fs::rename(source, backup) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            copy_dir_all(source, backup).map_err(|copy_error| {
                AppError::Conflict(format!(
                    "move to backup failed: {rename_error}; copy fallback failed: {copy_error}"
                ))
            })?;
            if let Err(remove_error) = fs::remove_dir_all(source) {
                let _ = fs::remove_dir_all(backup);
                return Err(remove_error.into());
            }
            Ok(())
        }
    }
}

fn move_symlink_to_backup(source: &Path, backup: &Path) -> Result<()> {
    let link = fs::read_link(source)?;
    match fs::rename(source, backup) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            symlink(&link, backup).map_err(|copy_error| {
                AppError::Conflict(format!(
                    "move symlink to backup failed: {rename_error}; symlink fallback failed: {copy_error}"
                ))
            })?;
            if let Err(remove_error) = fs::remove_file(source) {
                let _ = fs::remove_file(backup);
                return Err(remove_error.into());
            }
            Ok(())
        }
    }
}

fn restore_backup_to_source(backup: &Path, source: &Path) {
    if fs::rename(backup, source).is_ok() {
        return;
    }

    if let Ok(meta) = fs::symlink_metadata(backup) {
        if meta.file_type().is_symlink() {
            if let Ok(link) = fs::read_link(backup) {
                if symlink(link, source).is_ok() {
                    let _ = fs::remove_file(backup);
                }
            }
        }
    }
}

fn platform_roots_from_known() -> Vec<PlatformRoot> {
    PLATFORMS
        .iter()
        .filter(|platform| !platform.is_hub)
        .filter_map(|platform| {
            let root = expand_path(&platform.path)?;
            Some(PlatformRoot {
                id: platform.id.clone(),
                name: platform.name.clone(),
                path_label: platform.path.clone(),
                root,
            })
        })
        .collect()
}

fn make_item(
    candidate: &Candidate,
    central_root: &Path,
    action: InitAction,
    selected: bool,
    message: Option<String>,
) -> InitPreviewItem {
    InitPreviewItem {
        key: candidate.key.clone(),
        id: candidate.id.clone(),
        title: candidate.title.clone(),
        platform_id: candidate.platform.id.clone(),
        platform_name: candidate.platform.name.clone(),
        platform_path: candidate.platform.path_label.clone(),
        source_path: candidate.source.display().to_string(),
        content_path: candidate.content_source.display().to_string(),
        source_is_symlink: candidate.source_is_symlink,
        target_id: candidate.id.clone(),
        target_path: central_root.join(&candidate.id).display().to_string(),
        action,
        selected,
        message,
    }
}

fn invalid_item(
    platform: &PlatformRoot,
    id: String,
    source: PathBuf,
    central_root: &Path,
    message: String,
) -> InitPreviewItem {
    InitPreviewItem {
        key: item_key(&platform.id, &id),
        id: id.clone(),
        title: id.clone(),
        platform_id: platform.id.clone(),
        platform_name: platform.name.clone(),
        platform_path: platform.path_label.clone(),
        source_path: source.display().to_string(),
        content_path: source.display().to_string(),
        source_is_symlink: false,
        target_id: id.clone(),
        target_path: central_root.join(&id).display().to_string(),
        action: InitAction::SkipInvalid,
        selected: false,
        message: Some(message),
    }
}

fn already_routed_item(
    platform: &PlatformRoot,
    id: String,
    source: PathBuf,
    central_root: &Path,
) -> InitPreviewItem {
    InitPreviewItem {
        key: item_key(&platform.id, &id),
        id: id.clone(),
        title: extract_title_from_skill_md(&central_root.join(&id), &id),
        platform_id: platform.id.clone(),
        platform_name: platform.name.clone(),
        platform_path: platform.path_label.clone(),
        source_path: source.display().to_string(),
        content_path: central_root.join(&id).display().to_string(),
        source_is_symlink: true,
        target_id: id.clone(),
        target_path: central_root.join(&id).display().to_string(),
        action: InitAction::AlreadyRouted,
        selected: false,
        message: Some("Already points to the central repository.".into()),
    }
}

fn resolve_link_path(link: PathBuf, source: &Path) -> PathBuf {
    if link.is_absolute() {
        link
    } else {
        source.parent().map(|parent| parent.join(&link)).unwrap_or(link)
    }
}

fn candidate_from_skill_source(
    platform: &PlatformRoot,
    id: String,
    source: PathBuf,
    content_source: PathBuf,
    source_is_symlink: bool,
) -> Result<Candidate> {
    Ok(Candidate {
        key: item_key(&platform.id, &id),
        title: extract_title_from_skill_md(&content_source, &id),
        fingerprint: dir_fingerprint(&content_source)?,
        id,
        platform: platform.clone(),
        source,
        content_source,
        source_is_symlink,
    })
}

fn collect_platform_entries(
    central_root: &Path,
    platform: &PlatformRoot,
    candidates: &mut Vec<Candidate>,
    items: &mut Vec<InitPreviewItem>,
) -> Result<()> {
    if !platform.root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&platform.root)? {
        let entry = entry?;
        let source = entry.path();
        let id = entry.file_name().to_string_lossy().to_string();
        if validate_skill_id(&id).is_err() {
            items.push(invalid_item(
                platform,
                id,
                source,
                central_root,
                "Directory name is not a valid skill id.".into(),
            ));
            continue;
        }

        let meta = fs::symlink_metadata(&source)?;
        if meta.file_type().is_symlink() {
            let Ok(link) = fs::read_link(&source) else {
                items.push(invalid_item(
                    platform,
                    id,
                    source,
                    central_root,
                    "Could not read symlink target.".into(),
                ));
                continue;
            };

            let central_skill = central_root.join(&id);
            if let Ok(canonical_source) = central_skill.canonicalize() {
                if link_points_to_path(link.clone(), &source, &central_skill, &canonical_source) {
                    items.push(already_routed_item(platform, id, source, central_root));
                    continue;
                }
            }

            let resolved_link = resolve_link_path(link, &source);
            let Ok(content_source) = resolved_link.canonicalize() else {
                items.push(invalid_item(
                    platform,
                    id,
                    source,
                    central_root,
                    "Symlink target does not exist or is not readable.".into(),
                ));
                continue;
            };
            if !content_source.is_dir() {
                items.push(invalid_item(
                    platform,
                    id,
                    source,
                    central_root,
                    "Symlink target is not a skill directory.".into(),
                ));
                continue;
            }
            if !content_source.join("SKILL.md").is_file() {
                items.push(invalid_item(
                    platform,
                    id,
                    source,
                    central_root,
                    "Symlink target is missing SKILL.md.".into(),
                ));
                continue;
            }
            candidates.push(candidate_from_skill_source(
                platform,
                id,
                source,
                content_source,
                true,
            )?);
            continue;
        }

        if !meta.is_dir() {
            items.push(invalid_item(
                platform,
                id,
                source,
                central_root,
                "Entry is not a skill directory.".into(),
            ));
            continue;
        }

        if !source.join("SKILL.md").is_file() {
            items.push(invalid_item(
                platform,
                id,
                source,
                central_root,
                "Missing SKILL.md.".into(),
            ));
            continue;
        }

        candidates.push(candidate_from_skill_source(
            platform,
            id,
            source.clone(),
            source,
            false,
        )?);
    }

    Ok(())
}

fn build_preview_for_roots(central_root: &Path, platform_roots: &[PlatformRoot]) -> Result<InitPreview> {
    fs::create_dir_all(central_root)?;

    let mut candidates = Vec::new();
    let mut items = Vec::new();
    for platform in platform_roots {
        collect_platform_entries(central_root, platform, &mut candidates, &mut items)?;
    }

    let mut candidates_by_id: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for candidate in candidates {
        candidates_by_id
            .entry(candidate.id.clone())
            .or_default()
            .push(candidate);
    }

    for (id, mut group) in candidates_by_id {
        group.sort_by(|a, b| {
            a.platform
                .id
                .cmp(&b.platform.id)
                .then_with(|| a.source.cmp(&b.source))
        });

        let central_skill = central_root.join(&id);
        if central_skill.exists() {
            if !central_skill.is_dir() {
                for candidate in group {
                    items.push(make_item(
                        &candidate,
                        central_root,
                        InitAction::SkipConflict,
                        false,
                        Some("Central target already exists but is not a directory.".into()),
                    ));
                }
                continue;
            }

            let central_fingerprint = dir_fingerprint(&central_skill)?;
            for candidate in group {
                if candidate.fingerprint == central_fingerprint {
                    items.push(make_item(
                        &candidate,
                        central_root,
                        InitAction::LinkExistingCentral,
                        true,
                        Some("Central already has identical contents; source will be backed up and replaced with a route.".into()),
                    ));
                } else {
                    items.push(make_item(
                        &candidate,
                        central_root,
                        InitAction::SkipConflict,
                        false,
                        Some("Central already has a skill with different contents.".into()),
                    ));
                }
            }
            continue;
        }

        let unique_fingerprints: BTreeSet<&str> =
            group.iter().map(|candidate| candidate.fingerprint.as_str()).collect();
        if unique_fingerprints.len() > 1 {
            for candidate in group {
                items.push(make_item(
                    &candidate,
                    central_root,
                    InitAction::ResolveConflictSource,
                    false,
                    Some("Choose this source to copy into central; other conflicting sources will stay unchanged.".into()),
                ));
            }
            continue;
        }

        for (index, candidate) in group.iter().enumerate() {
            let action = if index == 0 {
                InitAction::MigrateToCentral
            } else {
                InitAction::LinkPlannedCentral
            };
            let message = if index == 0 && candidate.source_is_symlink {
                "Will copy linked source to central, then repoint this symlink to central."
            } else if index == 0 {
                "Will be copied to central, then replaced by a symlink route."
            } else if candidate.source_is_symlink {
                "Will be repointed to central after the identical linked source is migrated."
            } else {
                "Will be backed up and routed after the first identical copy is migrated."
            };
            items.push(make_item(
                candidate,
                central_root,
                action,
                true,
                Some(message.into()),
            ));
        }
    }

    items.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| a.platform_id.cmp(&b.platform_id))
            .then_with(|| a.key.cmp(&b.key))
    });
    let summary = summarize(&items);
    Ok(InitPreview {
        central_path: central_root.display().to_string(),
        items,
        summary,
    })
}

#[tauri::command]
pub fn preview_repository_init() -> Result<InitPreview> {
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    build_preview_for_roots(&central, &platform_roots_from_known())
}

fn backup_root() -> Result<PathBuf> {
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    let repo = central
        .parent()
        .ok_or_else(|| AppError::Conflict("central path has no parent".into()))?;
    let now = Local::now().format("%Y%m%d-%H%M%S");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis();
    Ok(repo.join("backups").join(format!("init-{now}-{nonce:03}")))
}

fn replace_source_with_route(item: &InitPreviewItem, central_root: &Path, backup_root: &Path) -> Result<()> {
    validate_skill_id(&item.id)?;
    let source = PathBuf::from(&item.source_path);
    let central_skill = central_root.join(&item.id);
    if !central_skill.is_dir() {
        return Err(AppError::SkillNotFound(item.id.clone()));
    }

    let meta = fs::symlink_metadata(&source)?;
    let backup = backup_root.join(&item.platform_id).join(&item.id);
    if backup.exists() {
        return Err(AppError::Conflict(format!(
            "backup target already exists: {}",
            backup.display()
        )));
    }
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }

    if meta.file_type().is_symlink() {
        move_symlink_to_backup(&source, &backup)?;
    } else if meta.is_dir() {
        move_dir_to_backup(&source, &backup)?;
    } else {
        return Err(AppError::Conflict(format!(
            "{} is no longer a skill directory or symlink",
            source.display()
        )));
    }

    if let Err(error) = symlink(&central_skill, &source) {
        restore_backup_to_source(&backup, &source);
        return Err(error.into());
    }

    Ok(())
}

fn execute_init_item(item: &InitPreviewItem, central_root: &Path, backup_root: &Path) -> Result<()> {
    match item.action {
        InitAction::MigrateToCentral | InitAction::ResolveConflictSource => {
            let source = PathBuf::from(&item.content_path);
            let central_skill = central_root.join(&item.id);
            if central_skill.exists() {
                return Err(AppError::Conflict(format!(
                    "{} already exists in central",
                    item.id
                )));
            }
            if let Err(error) = copy_dir_all(&source, &central_skill) {
                let _ = fs::remove_dir_all(&central_skill);
                return Err(error);
            }
            replace_source_with_route(item, central_root, backup_root)
        }
        InitAction::LinkExistingCentral | InitAction::LinkPlannedCentral => {
            replace_source_with_route(item, central_root, backup_root)
        }
        InitAction::AlreadyRouted | InitAction::SkipConflict | InitAction::SkipInvalid => Ok(()),
    }
}

#[tauri::command]
pub fn run_repository_init(selected_keys: Vec<String>) -> Result<InitResult> {
    let selected_keys: BTreeSet<String> = selected_keys.into_iter().collect();
    let preview = preview_repository_init()?;
    let central = PathBuf::from(&preview.central_path);
    let backup = backup_root()?;

    let mut runnable: Vec<InitPreviewItem> = preview
        .items
        .into_iter()
        .filter(|item| selected_keys.contains(&item.key) && selectable(&item.action))
        .collect();

    let mut conflict_source_counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in runnable
        .iter()
        .filter(|item| item.action == InitAction::ResolveConflictSource)
    {
        *conflict_source_counts.entry(item.id.clone()).or_default() += 1;
    }
    if let Some((id, _)) = conflict_source_counts
        .into_iter()
        .find(|(_, count)| *count > 1)
    {
        return Err(AppError::Conflict(format!(
            "select only one source for conflicting skill '{id}'"
        )));
    }

    runnable.sort_by_key(|item| match item.action {
        InitAction::MigrateToCentral | InitAction::ResolveConflictSource => 0,
        InitAction::LinkExistingCentral => 1,
        InitAction::LinkPlannedCentral => 2,
        InitAction::AlreadyRouted | InitAction::SkipConflict | InitAction::SkipInvalid => 3,
    });

    let mut result = InitResult {
        backup_root: backup.display().to_string(),
        completed: 0,
        skipped: 0,
        failed: 0,
        items: Vec::new(),
    };

    for item in runnable {
        match execute_init_item(&item, &central, &backup) {
            Ok(()) => {
                result.completed += 1;
                result.items.push(InitRunItem {
                    key: item.key,
                    id: item.id,
                    platform_id: item.platform_id,
                    action: item.action,
                    status: "completed".into(),
                    message: None,
                });
            }
            Err(error) => {
                result.failed += 1;
                result.items.push(InitRunItem {
                    key: item.key,
                    id: item.id,
                    platform_id: item.platform_id,
                    action: item.action,
                    status: "failed".into(),
                    message: Some(error.to_string()),
                });
            }
        }
    }

    if result.completed == 0 && result.failed == 0 {
        result.skipped = selected_keys.len();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    struct InitFixture {
        root: PathBuf,
        central: PathBuf,
        claude: PlatformRoot,
        codex: PlatformRoot,
    }

    impl InitFixture {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos();
            let root = env::temp_dir().join(format!(
                "skillloom-init-test-{}-{}-{}",
                std::process::id(),
                nonce,
                name
            ));
            let central = root.join("central");
            let claude_root = root.join("claude");
            let codex_root = root.join("codex");
            fs::create_dir_all(&central).expect("create central");
            fs::create_dir_all(&claude_root).expect("create claude");
            fs::create_dir_all(&codex_root).expect("create codex");

            Self {
                root,
                central,
                claude: PlatformRoot {
                    id: "claude".into(),
                    name: "Claude Code".into(),
                    path_label: "~/.claude/skills/".into(),
                    root: claude_root,
                },
                codex: PlatformRoot {
                    id: "codex".into(),
                    name: "Codex CLI".into(),
                    path_label: "~/.agents/skills/".into(),
                    root: codex_root,
                },
            }
        }

        fn write_skill(root: &Path, id: &str, body: &str) {
            let dir = root.join(id);
            fs::create_dir_all(&dir).expect("create skill");
            fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: {id}\ndescription: test\n---\n\n{body}\n"),
            )
            .expect("write skill md");
        }
    }

    impl Drop for InitFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn preview(fixture: &InitFixture) -> InitPreview {
        build_preview_for_roots(
            &fixture.central,
            &[fixture.claude.clone(), fixture.codex.clone()],
        )
        .expect("preview should build")
    }

    #[test]
    fn plans_single_real_skill_for_migration() {
        let fixture = InitFixture::new("single");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "from claude");

        let preview = preview(&fixture);

        assert_eq!(preview.summary.migratable, 1);
        assert_eq!(preview.items[0].id, "demo-skill");
        assert_eq!(preview.items[0].platform_id, "claude");
        assert_eq!(preview.items[0].action, InitAction::MigrateToCentral);
        assert!(preview.items[0].selected);
    }

    #[test]
    fn identical_platform_skills_use_one_copy_then_routes() {
        let fixture = InitFixture::new("identical");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "same");
        InitFixture::write_skill(&fixture.codex.root, "demo-skill", "same");

        let preview = preview(&fixture);
        let actions: Vec<_> = preview.items.iter().map(|item| item.action.clone()).collect();

        assert_eq!(
            actions,
            vec![InitAction::MigrateToCentral, InitAction::LinkPlannedCentral]
        );
        assert_eq!(preview.summary.migratable, 2);
    }

    #[test]
    fn conflicting_platform_skills_are_not_selected() {
        let fixture = InitFixture::new("conflict");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "one");
        InitFixture::write_skill(&fixture.codex.root, "demo-skill", "two");

        let preview = preview(&fixture);

        assert_eq!(preview.summary.conflicts, 2);
        assert!(preview.items.iter().all(|item| !item.selected));
        assert!(preview
            .items
            .iter()
            .all(|item| item.action == InitAction::ResolveConflictSource));
    }

    #[test]
    fn central_identical_skill_plans_route_replacement() {
        let fixture = InitFixture::new("central-identical");
        InitFixture::write_skill(&fixture.central, "demo-skill", "same");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "same");

        let preview = preview(&fixture);

        assert_eq!(preview.items[0].action, InitAction::LinkExistingCentral);
        assert!(preview.items[0].selected);
    }

    #[test]
    fn central_different_skill_is_conflict() {
        let fixture = InitFixture::new("central-different");
        InitFixture::write_skill(&fixture.central, "demo-skill", "central");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "platform");

        let preview = preview(&fixture);

        assert_eq!(preview.items[0].action, InitAction::SkipConflict);
        assert!(!preview.items[0].selected);
    }

    #[test]
    fn execute_migration_copies_to_central_and_replaces_source_with_route() {
        let fixture = InitFixture::new("execute-migration");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "from claude");
        fs::write(
            fixture.claude.root.join("demo-skill").join("notes.txt"),
            "extra file",
        )
        .expect("write extra file");

        let preview = preview(&fixture);
        let item = preview
            .items
            .into_iter()
            .find(|item| item.action == InitAction::MigrateToCentral)
            .expect("migration item");
        let backup = fixture.root.join("backup");

        execute_init_item(&item, &fixture.central, &backup).expect("execute migration");

        assert!(fixture.central.join("demo-skill").join("SKILL.md").is_file());
        assert!(fixture.central.join("demo-skill").join("notes.txt").is_file());
        assert!(backup
            .join("claude")
            .join("demo-skill")
            .join("SKILL.md")
            .is_file());

        let source = fixture.claude.root.join("demo-skill");
        let source_meta = fs::symlink_metadata(&source).expect("source route metadata");
        assert!(source_meta.file_type().is_symlink());
        assert_eq!(
            fs::read_link(source).expect("read source route"),
            fixture.central.join("demo-skill")
        );
    }

    #[test]
    fn external_symlink_to_skill_directory_is_migratable() {
        let fixture = InitFixture::new("external-symlink");
        let external_root = fixture.root.join("cc-switch");
        InitFixture::write_skill(&external_root, "ai-trends", "linked source");
        let external_skill = external_root.join("ai-trends");
        symlink(&external_skill, fixture.claude.root.join("ai-trends"))
            .expect("create external skill symlink");

        let preview = preview(&fixture);
        let item = preview
            .items
            .iter()
            .find(|item| item.id == "ai-trends")
            .expect("linked item");

        assert_eq!(item.action, InitAction::MigrateToCentral);
        assert!(item.selected);
        assert!(item.source_is_symlink);
        assert_eq!(
            PathBuf::from(&item.content_path),
            external_skill.canonicalize().expect("canonical external skill")
        );
        assert!(item
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("linked source"));
    }

    #[test]
    fn execute_external_symlink_migration_copies_target_and_repoints_link() {
        let fixture = InitFixture::new("execute-external-symlink");
        let external_root = fixture.root.join("cc-switch");
        InitFixture::write_skill(&external_root, "ai-trends", "linked source");
        let external_skill = external_root.join("ai-trends");
        fs::write(external_skill.join("notes.txt"), "linked note").expect("write linked note");
        let platform_link = fixture.claude.root.join("ai-trends");
        symlink(&external_skill, &platform_link).expect("create external skill symlink");

        let preview = preview(&fixture);
        let item = preview
            .items
            .into_iter()
            .find(|item| item.id == "ai-trends")
            .expect("linked item");
        let backup = fixture.root.join("backup");

        execute_init_item(&item, &fixture.central, &backup).expect("execute linked migration");

        assert!(fixture.central.join("ai-trends").join("SKILL.md").is_file());
        assert!(fixture.central.join("ai-trends").join("notes.txt").is_file());
        assert!(external_skill.join("SKILL.md").is_file());

        let route_meta = fs::symlink_metadata(&platform_link).expect("route metadata");
        assert!(route_meta.file_type().is_symlink());
        assert_eq!(
            fs::read_link(&platform_link).expect("read central route"),
            fixture.central.join("ai-trends")
        );

        let backup_link = backup.join("claude").join("ai-trends");
        let backup_meta = fs::symlink_metadata(&backup_link).expect("backup link metadata");
        assert!(backup_meta.file_type().is_symlink());
        assert_eq!(
            fs::read_link(backup_link).expect("read backed up external link"),
            external_skill
        );
    }

    #[test]
    fn selected_conflict_source_migrates_only_that_platform() {
        let fixture = InitFixture::new("execute-conflict-source");
        InitFixture::write_skill(&fixture.claude.root, "gsd-schema", "claude version");
        InitFixture::write_skill(&fixture.codex.root, "gsd-schema", "codex version");

        let preview = preview(&fixture);
        let item = preview
            .items
            .into_iter()
            .find(|item| item.platform_id == "claude" && item.id == "gsd-schema")
            .expect("claude conflict source");
        let backup = fixture.root.join("backup");

        assert_eq!(item.action, InitAction::ResolveConflictSource);
        execute_init_item(&item, &fixture.central, &backup)
            .expect("execute conflict source migration");

        let central_md = fs::read_to_string(fixture.central.join("gsd-schema").join("SKILL.md"))
            .expect("central skill md");
        assert!(central_md.contains("claude version"));

        let claude_entry = fixture.claude.root.join("gsd-schema");
        assert!(fs::symlink_metadata(&claude_entry)
            .expect("claude route metadata")
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_link(claude_entry).expect("read claude route"),
            fixture.central.join("gsd-schema")
        );

        assert!(fixture.codex.root.join("gsd-schema").join("SKILL.md").is_file());
        let codex_md = fs::read_to_string(fixture.codex.root.join("gsd-schema").join("SKILL.md"))
            .expect("codex skill md");
        assert!(codex_md.contains("codex version"));
    }
}
