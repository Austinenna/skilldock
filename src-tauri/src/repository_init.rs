use crate::error::{AppError, Result};
use crate::platforms::{central_dir, expand_path, PLATFORMS};
use crate::skills::{link_points_to_path, validate_skill_id};
use chrono::Local;
use serde::Serialize;
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
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
    pub fingerprint: String,
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

#[derive(Debug)]
enum CentralTargetState {
    Missing,
    Directory { path: PathBuf, canonical: PathBuf },
    Symlink,
    Other,
    Outside,
}

#[derive(Debug, Serialize)]
struct InitJournalEntry {
    timestamp: String,
    phase: String,
    key: Option<String>,
    id: Option<String>,
    platform_id: Option<String>,
    action: Option<String>,
    fingerprint: Option<String>,
    source_path: Option<String>,
    central_path: Option<String>,
    backup_path: Option<String>,
    message: Option<String>,
}

struct InitJournal {
    path: PathBuf,
}

impl InitJournal {
    fn create(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let path = root.join("journal.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        file.sync_all()?;
        Ok(Self { path })
    }

    fn record(
        &self,
        phase: &str,
        item: Option<&InitPreviewItem>,
        backup_root: &Path,
        message: Option<String>,
    ) -> Result<()> {
        let entry = InitJournalEntry {
            timestamp: Local::now().to_rfc3339(),
            phase: phase.into(),
            key: item.map(|value| value.key.clone()),
            id: item.map(|value| value.id.clone()),
            platform_id: item.map(|value| value.platform_id.clone()),
            action: item.map(|value| format!("{:?}", value.action)),
            fingerprint: item.map(|value| value.fingerprint.clone()),
            source_path: item.map(|value| value.source_path.clone()),
            central_path: item.map(|value| value.target_path.clone()),
            backup_path: item.map(|value| {
                backup_root
                    .join(&value.platform_id)
                    .join(&value.id)
                    .display()
                    .to_string()
            }),
            message,
        };
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        serde_json::to_writer(&mut file, &entry)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(())
    }
}

fn central_target_state(central_root: &Path, id: &str) -> Result<CentralTargetState> {
    validate_skill_id(id)?;
    let path = central_root.join(id);
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CentralTargetState::Missing)
        }
        Err(error) => return Err(error.into()),
    };

    if meta.file_type().is_symlink() {
        return Ok(CentralTargetState::Symlink);
    }
    if !meta.is_dir() {
        return Ok(CentralTargetState::Other);
    }

    let canonical_root = central_root.canonicalize()?;
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&canonical_root) {
        return Ok(CentralTargetState::Outside);
    }

    Ok(CentralTargetState::Directory { path, canonical })
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

fn validate_expected_fingerprints(
    preview: &InitPreview,
    selected_keys: &BTreeSet<String>,
    expected_fingerprints: &BTreeMap<String, String>,
) -> Result<()> {
    for key in selected_keys {
        let item = preview
            .items
            .iter()
            .find(|item| &item.key == key)
            .ok_or_else(|| AppError::Conflict(format!("preview item no longer exists: {key}")))?;
        if !selectable(&item.action) {
            return Err(AppError::Conflict(format!(
                "preview item is not selectable: {key}"
            )));
        }
        let expected = expected_fingerprints.get(key).ok_or_else(|| {
            AppError::Conflict(format!(
                "preview fingerprint is missing; rescan before migrating: {key}"
            ))
        })?;
        if expected != &item.fingerprint {
            return Err(AppError::Conflict(format!(
                "skill source changed after preview; rescan before migrating: {key}"
            )));
        }
    }
    Ok(())
}

fn verify_item_fingerprint(item: &InitPreviewItem) -> Result<()> {
    let source_path = PathBuf::from(&item.source_path);
    let source_meta = fs::symlink_metadata(&source_path)?;
    if source_meta.file_type().is_symlink() != item.source_is_symlink {
        return Err(AppError::Conflict(format!(
            "skill source type changed during migration: {}",
            item.key
        )));
    }
    let content_path = PathBuf::from(&item.content_path);
    if item.source_is_symlink {
        let link = fs::read_link(&source_path)?;
        let canonical_content = content_path.canonicalize()?;
        if !link_points_to_path(link, &source_path, &content_path, &canonical_content) {
            return Err(AppError::Conflict(format!(
                "skill source symlink target changed during migration: {}",
                item.key
            )));
        }
    }
    if !content_path.is_dir() {
        return Err(AppError::Conflict(format!(
            "skill source is no longer a directory: {}",
            content_path.display()
        )));
    }
    let actual = dir_fingerprint(&content_path)?;
    if actual != item.fingerprint {
        return Err(AppError::Conflict(format!(
            "skill source changed during migration: {}",
            item.key
        )));
    }
    Ok(())
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
    let mut entries = match fs::read_dir(source)
        .and_then(|entries| entries.collect::<std::result::Result<Vec<_>, _>>())
    {
        Ok(entries) => entries,
        Err(error) => {
            let _ = fs::remove_dir_all(dest);
            return Err(error.into());
        }
    };
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let src = entry.path();
        let dst = dest.join(entry.file_name());
        let meta = match fs::symlink_metadata(&src) {
            Ok(meta) => meta,
            Err(error) => {
                let _ = fs::remove_dir_all(dest);
                return Err(error.into());
            }
        };
        let result = if meta.is_dir() {
            copy_dir_all(&src, &dst)
        } else if meta.is_file() {
            fs::copy(&src, &dst).map(|_| ()).map_err(AppError::from)
        } else if meta.file_type().is_symlink() {
            symlink(fs::read_link(&src)?, &dst).map_err(AppError::from)
        } else {
            Ok(())
        };
        if let Err(error) = result {
            let _ = fs::remove_dir_all(dest);
            return Err(error);
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
                return Err(AppError::Conflict(format!(
                    "source removal after backup failed; backup was preserved at {}: {remove_error}",
                    backup.display()
                )));
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
                return Err(AppError::Conflict(format!(
                    "source removal after symlink backup failed; backup was preserved at {}: {remove_error}",
                    backup.display()
                )));
            }
            Ok(())
        }
    }
}

fn restore_backup_to_source(backup: &Path, source: &Path) -> Result<()> {
    if fs::symlink_metadata(source).is_ok() {
        return Err(AppError::Conflict(format!(
            "cannot restore backup because source path already exists: {}",
            source.display()
        )));
    }

    let meta = fs::symlink_metadata(backup)?;
    if meta.file_type().is_symlink() {
        let link = fs::read_link(backup)?;
        symlink(link, source)?;
        return Ok(());
    }
    if meta.is_dir() {
        copy_dir_all(backup, source)?;
        return Ok(());
    }

    Err(AppError::Conflict(format!(
        "backup is neither a directory nor a symlink: {}",
        backup.display()
    )))
}

fn remove_created_central_skill(central_root: &Path, item: &InitPreviewItem) -> Result<()> {
    let path = match central_target_state(central_root, &item.id)? {
        CentralTargetState::Missing => return Ok(()),
        CentralTargetState::Directory { path, .. } => path,
        CentralTargetState::Symlink | CentralTargetState::Other | CentralTargetState::Outside => {
            return Err(AppError::Conflict(format!(
                "central copy for '{}' is unsafe during recovery",
                item.id
            )))
        }
    };
    if dir_fingerprint(&path)? != item.fingerprint {
        return Err(AppError::Conflict(format!(
            "central copy for '{}' changed during recovery",
            item.id
        )));
    }
    if central_has_other_route(central_root, item, &[]) {
        return Err(AppError::Conflict(format!(
            "central copy for '{}' is still referenced by another platform route",
            item.id
        )));
    }
    fs::remove_dir_all(path)?;
    Ok(())
}

fn central_has_other_route(
    central_root: &Path,
    item: &InitPreviewItem,
    related_items: &[InitPreviewItem],
) -> bool {
    let canonical = central_root.join(&item.id).canonicalize().ok();
    let Some(canonical) = canonical else {
        return false;
    };
    let mut sources = related_items
        .iter()
        .filter(|candidate| candidate.id == item.id)
        .map(|candidate| PathBuf::from(&candidate.source_path))
        .collect::<Vec<_>>();
    sources.extend(
        platform_roots_from_known()
            .into_iter()
            .map(|platform| platform.root.join(&item.id)),
    );
    sources.into_iter().any(|source| {
        if source == PathBuf::from(&item.source_path) {
            return false;
        }
        let Ok(meta) = fs::symlink_metadata(&source) else {
            return false;
        };
        if !meta.file_type().is_symlink() {
            return false;
        }
        let Ok(link) = fs::read_link(&source) else {
            return false;
        };
        link_points_to_path(link, &source, &central_root.join(&item.id), &canonical)
    })
}

fn restore_completed_item(
    item: &InitPreviewItem,
    central_root: &Path,
    backup_root: &Path,
    related_items: &[InitPreviewItem],
) -> Result<()> {
    let source = PathBuf::from(&item.source_path);
    let central_skill = match central_target_state(central_root, &item.id)? {
        CentralTargetState::Directory { path, canonical } => (path, canonical),
        CentralTargetState::Missing => {
            return Err(AppError::Conflict(format!(
                "central target for '{}' is missing during recovery",
                item.id
            )))
        }
        CentralTargetState::Symlink | CentralTargetState::Other | CentralTargetState::Outside => {
            return Err(AppError::Conflict(format!(
                "central target for '{}' is unsafe during recovery",
                item.id
            )))
        }
    };
    let meta = fs::symlink_metadata(&source)?;
    if !meta.file_type().is_symlink() {
        return Err(AppError::Conflict(format!(
            "source changed during recovery: {}",
            source.display()
        )));
    }
    let link = fs::read_link(&source)?;
    if !link_points_to_path(link, &source, &central_skill.0, &central_skill.1) {
        return Err(AppError::Conflict(format!(
            "source no longer points to central during recovery: {}",
            source.display()
        )));
    }
    fs::remove_file(&source)?;
    let backup = backup_root.join(&item.platform_id).join(&item.id);
    restore_backup_to_source(&backup, &source)?;

    if matches!(
        item.action,
        InitAction::MigrateToCentral | InitAction::ResolveConflictSource
    ) {
        if central_has_other_route(central_root, item, related_items) {
            return Err(AppError::Conflict(format!(
                "source restored but central copy for '{}' remains referenced by another route",
                item.id
            )));
        }
        remove_created_central_skill(central_root, item)?;
    }
    Ok(())
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
        fingerprint: candidate.fingerprint.clone(),
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
        fingerprint: String::new(),
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
        fingerprint: String::new(),
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
        source
            .parent()
            .map(|parent| parent.join(&link))
            .unwrap_or(link)
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

            if let Ok(CentralTargetState::Directory {
                path: central_skill,
                canonical,
            }) = central_target_state(central_root, &id)
            {
                if link_points_to_path(link.clone(), &source, &central_skill, &canonical) {
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

fn build_preview_for_roots(
    central_root: &Path,
    platform_roots: &[PlatformRoot],
) -> Result<InitPreview> {
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

        let central_skill = match central_target_state(central_root, &id)? {
            CentralTargetState::Missing => None,
            CentralTargetState::Directory { path, canonical: _ } => Some(path),
            CentralTargetState::Symlink
            | CentralTargetState::Other
            | CentralTargetState::Outside => {
                for candidate in group {
                    items.push(make_item(
                        &candidate,
                        central_root,
                        InitAction::SkipConflict,
                        false,
                        Some(
                            "Central target is not a safe real directory; migration is blocked."
                                .into(),
                        ),
                    ));
                }
                continue;
            }
        };

        if let Some(central_skill) = central_skill {
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

        let unique_fingerprints: BTreeSet<&str> = group
            .iter()
            .map(|candidate| candidate.fingerprint.as_str())
            .collect();
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
    let backups = repo.join("backups");
    fs::create_dir_all(&backups)?;
    let now = Local::now().format("%Y%m%d-%H%M%S");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..100u32 {
        let candidate = backups.join(format!("init-{now}-{nonce}-{attempt:02}"));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(AppError::Conflict(
        "could not allocate a unique initialization backup directory".into(),
    ))
}

fn replace_source_with_route_logged(
    item: &InitPreviewItem,
    central_root: &Path,
    backup_root: &Path,
    journal: Option<&InitJournal>,
) -> Result<()> {
    validate_skill_id(&item.id)?;
    let source = PathBuf::from(&item.source_path);
    let central_skill = match central_target_state(central_root, &item.id)? {
        CentralTargetState::Directory { path, canonical: _ } => path,
        CentralTargetState::Missing => return Err(AppError::SkillNotFound(item.id.clone())),
        CentralTargetState::Symlink | CentralTargetState::Other | CentralTargetState::Outside => {
            return Err(AppError::Conflict(format!(
                "central target for '{}' is not a safe real directory",
                item.id
            )))
        }
    };

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

    if let Some(journal) = journal {
        journal.record("source_backup_started", Some(item), backup_root, None)?;
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

    if let Some(journal) = journal {
        if let Err(error) = journal.record("source_backed_up", Some(item), backup_root, None) {
            let restore_error = restore_backup_to_source(&backup, &source).err();
            return Err(AppError::Conflict(format!(
                "journal write failed after backup: {error}{}",
                restore_error
                    .map(|value| format!("; source restore failed: {value}"))
                    .unwrap_or_else(|| "; source restored".into())
            )));
        }
        if let Err(error) = journal.record("route_create_started", Some(item), backup_root, None) {
            let restore_error = restore_backup_to_source(&backup, &source).err();
            return Err(AppError::Conflict(format!(
                "journal write failed before route creation: {error}{}",
                restore_error
                    .map(|value| format!("; source restore failed: {value}"))
                    .unwrap_or_else(|| "; source restored".into())
            )));
        }
    }

    if let Err(error) = symlink(&central_skill, &source) {
        let restore_error = restore_backup_to_source(&backup, &source).err();
        if let Some(journal) = journal {
            let message = restore_error
                .as_ref()
                .map(|restore_error| {
                    format!("route error: {error}; restore error: {restore_error}")
                })
                .unwrap_or_else(|| format!("route error: {error}; source restored"));
            let journal_error = journal
                .record(
                    "source_restored_after_route_failure",
                    Some(item),
                    backup_root,
                    Some(message),
                )
                .err();
            if let Some(journal_error) = journal_error {
                return Err(AppError::Conflict(format!(
                    "creating route failed: {error}; journal write failed: {journal_error}"
                )));
            }
        }
        return Err(match restore_error {
            Some(restore_error) => AppError::Conflict(format!(
                "creating route failed: {error}; restoring source failed: {restore_error}"
            )),
            None => error.into(),
        });
    }

    if let Some(journal) = journal {
        if let Err(error) = journal.record("route_created", Some(item), backup_root, None) {
            let route_removed = fs::symlink_metadata(&source)
                .ok()
                .filter(|meta| meta.file_type().is_symlink())
                .and_then(|_| fs::read_link(&source).ok())
                .filter(|link| {
                    link_points_to_path(
                        link.clone(),
                        &source,
                        &central_skill,
                        &central_skill
                            .canonicalize()
                            .unwrap_or_else(|_| central_skill.clone()),
                    )
                })
                .and_then(|_| fs::remove_file(&source).ok())
                .is_some();
            let restore_error = restore_backup_to_source(&backup, &source).err();
            return Err(AppError::Conflict(format!(
                "journal write failed after route creation: {error}; route removed={route_removed}{}",
                restore_error
                    .map(|value| format!("; source restore failed: {value}"))
                    .unwrap_or_else(|| "; source restored".into())
            )));
        }
    }

    Ok(())
}

fn execute_init_item_logged(
    item: &InitPreviewItem,
    central_root: &Path,
    backup_root: &Path,
    journal: Option<&InitJournal>,
) -> Result<()> {
    if selectable(&item.action) {
        verify_item_fingerprint(item)?;
    }
    if let Some(journal) = journal {
        journal.record("item_started", Some(item), backup_root, None)?;
    }

    let result = match item.action {
        InitAction::MigrateToCentral | InitAction::ResolveConflictSource => {
            let source = PathBuf::from(&item.content_path);
            if !matches!(
                central_target_state(central_root, &item.id)?,
                CentralTargetState::Missing
            ) {
                return Err(AppError::Conflict(format!(
                    "{} already exists in central or is unsafe",
                    item.id
                )));
            }
            let central_skill = central_root.join(&item.id);
            if let Some(journal) = journal {
                journal.record("central_copy_started", Some(item), backup_root, None)?;
            }
            if let Err(error) = copy_dir_all(&source, &central_skill) {
                return Err(error);
            }
            if dir_fingerprint(&central_skill)? != item.fingerprint {
                let cleanup = remove_created_central_skill(central_root, item).err();
                return Err(AppError::Conflict(format!(
                    "source changed while copying '{}'; central copy was{} removed{}",
                    item.id,
                    if cleanup.is_none() { "" } else { " not" },
                    cleanup
                        .map(|value| format!(" ({value})"))
                        .unwrap_or_default()
                )));
            }
            if let Some(journal) = journal {
                if let Err(error) = journal.record("central_copied", Some(item), backup_root, None)
                {
                    let cleanup = remove_created_central_skill(central_root, item).err();
                    return Err(AppError::Conflict(format!(
                        "journal write failed after central copy: {error}{}",
                        cleanup
                            .map(|value| format!("; central cleanup failed: {value}"))
                            .unwrap_or_else(|| "; central copy removed".into())
                    )));
                }
            }
            if let Err(error) =
                replace_source_with_route_logged(item, central_root, backup_root, journal)
            {
                let cleanup_error = remove_created_central_skill(central_root, item).err();
                if let Some(journal) = journal {
                    let journal_error = journal
                        .record(
                            "central_removed_after_failure",
                            Some(item),
                            backup_root,
                            Some(
                                cleanup_error
                                    .as_ref()
                                    .map(|value| {
                                        format!("{error}; central cleanup failed: {value}")
                                    })
                                    .unwrap_or_else(|| error.to_string()),
                            ),
                        )
                        .err();
                    if let Some(journal_error) = journal_error {
                        return Err(AppError::Conflict(format!(
                            "{error}; central cleanup result={}; journal write failed: {journal_error}",
                            cleanup_error
                                .as_ref()
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "removed".into())
                        )));
                    }
                }
                return Err(match cleanup_error {
                    Some(cleanup_error) => AppError::Conflict(format!(
                        "{error}; central cleanup failed: {cleanup_error}"
                    )),
                    None => error,
                });
            }
            Ok(())
        }
        InitAction::LinkExistingCentral | InitAction::LinkPlannedCentral => {
            let CentralTargetState::Directory { path, .. } =
                central_target_state(central_root, &item.id)?
            else {
                return Err(AppError::Conflict(format!(
                    "central target for '{}' is not a safe real directory",
                    item.id
                )));
            };
            if dir_fingerprint(&path)? != item.fingerprint {
                return Err(AppError::Conflict(format!(
                    "central skill changed during migration: {}",
                    item.key
                )));
            }
            replace_source_with_route_logged(item, central_root, backup_root, journal)
        }
        InitAction::AlreadyRouted | InitAction::SkipConflict | InitAction::SkipInvalid => Ok(()),
    };

    if result.is_ok() {
        if let Some(journal) = journal {
            if let Err(error) = journal.record("item_completed", Some(item), backup_root, None) {
                let recovery = restore_completed_item(item, central_root, backup_root, &[]).err();
                return Err(AppError::Conflict(format!(
                    "journal write failed after item completion: {error}{}",
                    recovery
                        .map(|value| format!("; rollback failed: {value}"))
                        .unwrap_or_else(|| "; item rolled back".into())
                )));
            }
        }
    }
    result
}

#[cfg(test)]
fn execute_init_item(
    item: &InitPreviewItem,
    central_root: &Path,
    backup_root: &Path,
) -> Result<()> {
    execute_init_item_logged(item, central_root, backup_root, None)
}

fn rollback_completed_items(
    items: &[InitPreviewItem],
    central_root: &Path,
    backup_root: &Path,
    journal: &InitJournal,
) -> Vec<String> {
    let mut errors = Vec::new();
    for item in items.iter().rev() {
        let _ = journal.record("rollback_started", Some(item), backup_root, None);
        match restore_completed_item(item, central_root, backup_root, items) {
            Ok(()) => {
                let _ = journal.record("rollback_completed", Some(item), backup_root, None);
            }
            Err(error) => {
                let message = format!("{}/{}: {error}", item.platform_id, item.id);
                let _ = journal.record(
                    "rollback_failed",
                    Some(item),
                    backup_root,
                    Some(message.clone()),
                );
                errors.push(message);
            }
        }
    }
    errors
}

#[tauri::command]
pub fn run_repository_init(
    selected_keys: Vec<String>,
    expected_fingerprints: BTreeMap<String, String>,
) -> Result<InitResult> {
    let selected_keys: BTreeSet<String> = selected_keys.into_iter().collect();
    let preview = preview_repository_init()?;
    validate_expected_fingerprints(&preview, &selected_keys, &expected_fingerprints)?;
    let central = PathBuf::from(&preview.central_path);

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

    if runnable.is_empty() {
        return Ok(InitResult {
            backup_root: String::new(),
            completed: 0,
            skipped: selected_keys.len(),
            failed: 0,
            items: Vec::new(),
        });
    }

    let backup = backup_root()?;
    let journal = InitJournal::create(&backup)?;
    journal.record(
        "run_started",
        None,
        &backup,
        Some(format!("selected_items={}", selected_keys.len())),
    )?;

    let mut result = InitResult {
        backup_root: backup.display().to_string(),
        completed: 0,
        skipped: 0,
        failed: 0,
        items: Vec::new(),
    };

    let mut completed_items = Vec::new();
    for (index, item) in runnable.iter().enumerate() {
        match execute_init_item_logged(item, &central, &backup, Some(&journal)) {
            Ok(()) => {
                result.completed += 1;
                completed_items.push(item.clone());
                result.items.push(InitRunItem {
                    key: item.key.clone(),
                    id: item.id.clone(),
                    platform_id: item.platform_id.clone(),
                    action: item.action.clone(),
                    status: "completed".into(),
                    message: None,
                });
            }
            Err(error) => {
                let rollback_errors =
                    rollback_completed_items(&completed_items, &central, &backup, &journal);
                let mut message = error.to_string();
                if !rollback_errors.is_empty() {
                    message.push_str("; rollback failed: ");
                    message.push_str(&rollback_errors.join("; "));
                }
                for previous in &mut result.items {
                    if previous.status == "completed" {
                        let prefix = format!("{}/{}:", previous.platform_id, previous.id);
                        if rollback_errors
                            .iter()
                            .any(|value| value.starts_with(&prefix))
                        {
                            previous.status = "rollback-failed".into();
                        } else {
                            previous.status = "rolled-back".into();
                        }
                    }
                }
                result.completed = 0;
                result.failed += 1 + rollback_errors.len();
                result.items.push(InitRunItem {
                    key: item.key.clone(),
                    id: item.id.clone(),
                    platform_id: item.platform_id.clone(),
                    action: item.action.clone(),
                    status: "failed".into(),
                    message: Some(message),
                });
                result.skipped += runnable.len().saturating_sub(index + 1);
                let _ = journal.record("run_failed", None, &backup, Some(error.to_string()));
                break;
            }
        }
    }

    if result.completed == 0 && result.failed == 0 {
        result.skipped = selected_keys.len();
    }

    if result.failed == 0 {
        if let Err(error) = journal.record("run_completed", None, &backup, None) {
            let rollback_errors =
                rollback_completed_items(&completed_items, &central, &backup, &journal);
            for previous in &mut result.items {
                let prefix = format!("{}/{}:", previous.platform_id, previous.id);
                if rollback_errors
                    .iter()
                    .any(|value| value.starts_with(&prefix))
                {
                    previous.status = "rollback-failed".into();
                } else {
                    previous.status = "rolled-back".into();
                }
            }
            result.completed = 0;
            result.failed = 1 + rollback_errors.len();
            let mut message = format!("completion journal write failed: {error}");
            if !rollback_errors.is_empty() {
                message.push_str("; rollback failed: ");
                message.push_str(&rollback_errors.join("; "));
            } else {
                message.push_str("; completed items rolled back");
            }
            if let Some(first) = result.items.first_mut() {
                first.message = Some(message);
            }
        }
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
                "skilldock-init-test-{}-{}-{}",
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
                    path_label: "~/.codex/skills/".into(),
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
        let actions: Vec<_> = preview
            .items
            .iter()
            .map(|item| item.action.clone())
            .collect();

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
    fn central_symlink_target_is_blocked() {
        let fixture = InitFixture::new("central-symlink");
        let external_root = fixture.root.join("external");
        InitFixture::write_skill(&external_root, "demo-skill", "external");
        symlink(
            external_root.join("demo-skill"),
            fixture.central.join("demo-skill"),
        )
        .expect("create central symlink");
        let platform_skill = fixture.claude.root.join("demo-skill");
        symlink(fixture.central.join("demo-skill"), &platform_skill)
            .expect("create platform route to unsafe central target");

        let preview = preview(&fixture);

        assert_eq!(preview.items[0].action, InitAction::SkipConflict);
        assert!(!preview.items[0].selected);
        assert!(preview.items[0].source_is_symlink);
        assert!(preview.items[0]
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("safe real directory"));
    }

    #[test]
    fn changed_source_fingerprint_is_rejected_after_preview() {
        let fixture = InitFixture::new("fingerprint-change");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "before");
        let first_preview = preview(&fixture);
        let item = first_preview.items[0].clone();
        let selected = BTreeSet::from([item.key.clone()]);
        let expected = BTreeMap::from([(item.key.clone(), item.fingerprint.clone())]);

        fs::write(
            fixture.claude.root.join("demo-skill").join("SKILL.md"),
            "---\nname: demo-skill\ndescription: changed\n---\n\nafter\n",
        )
        .expect("change source");

        let changed_preview = preview(&fixture);
        let error = validate_expected_fingerprints(&changed_preview, &selected, &expected)
            .expect_err("changed source should invalidate preview");
        assert!(error.to_string().contains("changed after preview"));
    }

    #[test]
    fn migration_writes_transaction_journal() {
        let fixture = InitFixture::new("journal");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "journal");
        let item = preview(&fixture).items[0].clone();
        let backup = fixture.root.join("backup");
        let journal = InitJournal::create(&backup).expect("create journal");

        execute_init_item_logged(&item, &fixture.central, &backup, Some(&journal))
            .expect("execute migration with journal");

        let contents = fs::read_to_string(backup.join("journal.jsonl")).expect("read journal");
        assert!(contents.contains("central_copied"));
        assert!(contents.contains("source_backed_up"));
        assert!(contents.contains("route_created"));
        assert!(contents.contains("item_completed"));
    }

    #[test]
    fn completed_migration_can_be_rolled_back() {
        let fixture = InitFixture::new("rollback");
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "rollback");
        let item = preview(&fixture).items[0].clone();
        let backup = fixture.root.join("backup");
        let journal = InitJournal::create(&backup).expect("create journal");

        execute_init_item_logged(&item, &fixture.central, &backup, Some(&journal))
            .expect("execute migration");
        rollback_completed_items(&[item], &fixture.central, &backup, &journal);

        assert!(fixture
            .claude
            .root
            .join("demo-skill")
            .join("SKILL.md")
            .is_file());
        assert!(!fixture.central.join("demo-skill").exists());
    }

    #[test]
    fn restore_refuses_existing_source_and_keeps_backup() {
        let fixture = InitFixture::new("restore-existing");
        let backup = fixture
            .root
            .join("backup")
            .join("claude")
            .join("demo-skill");
        InitFixture::write_skill(
            &fixture.root.join("backup").join("claude"),
            "demo-skill",
            "backup",
        );
        InitFixture::write_skill(&fixture.claude.root, "demo-skill", "new source");

        let error = restore_backup_to_source(&backup, &fixture.claude.root.join("demo-skill"))
            .expect_err("restore must not overwrite a new source");
        assert!(error.to_string().contains("already exists"));
        assert!(backup.join("SKILL.md").is_file());
        assert!(
            fs::read_to_string(fixture.claude.root.join("demo-skill").join("SKILL.md"))
                .expect("source remains")
                .contains("new source")
        );
    }

    #[test]
    fn failed_copy_does_not_remove_existing_central_target() {
        let fixture = InitFixture::new("copy-existing-target");
        let source = fixture.root.join("source");
        InitFixture::write_skill(&source, "demo-skill", "source");
        InitFixture::write_skill(&fixture.central, "demo-skill", "existing central");

        let error = copy_dir_all(
            &source.join("demo-skill"),
            &fixture.central.join("demo-skill"),
        )
        .expect_err("copy must refuse an existing target");
        assert!(error.to_string().contains("File exists") || error.to_string().contains("exists"));
        assert!(
            fs::read_to_string(fixture.central.join("demo-skill").join("SKILL.md"))
                .expect("central remains")
                .contains("existing central")
        );
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

        assert!(fixture
            .central
            .join("demo-skill")
            .join("SKILL.md")
            .is_file());
        assert!(fixture
            .central
            .join("demo-skill")
            .join("notes.txt")
            .is_file());
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
            external_skill
                .canonicalize()
                .expect("canonical external skill")
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
        assert!(fixture
            .central
            .join("ai-trends")
            .join("notes.txt")
            .is_file());
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

        assert!(fixture
            .codex
            .root
            .join("gsd-schema")
            .join("SKILL.md")
            .is_file());
        let codex_md = fs::read_to_string(fixture.codex.root.join("gsd-schema").join("SKILL.md"))
            .expect("codex skill md");
        assert!(codex_md.contains("codex version"));
    }
}
