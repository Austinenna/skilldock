use crate::error::{AppError, Result};
use crate::platforms::{central_dir, platform_by_id, platform_dir};
use std::fs;
use std::os::unix::fs::symlink;

#[tauri::command]
pub fn add_route(skill_id: String, platform_id: String) -> Result<()> {
    let platform = platform_by_id(&platform_id)
        .ok_or_else(|| AppError::PlatformNotFound(platform_id.clone()))?;
    if platform.is_hub { return Err(AppError::HubRoute); }

    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    let source = central.join(&skill_id);
    if !source.exists() {
        return Err(AppError::SkillNotFound(skill_id));
    }

    let platform_root = platform_dir(&platform_id).ok_or(AppError::NoHomeDir)?;
    fs::create_dir_all(&platform_root)?;
    let target = platform_root.join(&skill_id);

    // If something already exists at the target, decide carefully.
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.file_type().is_symlink() {
            // Idempotent: same symlink to our central is fine.
            if let Ok(link) = fs::read_link(&target) {
                let resolved = if link.is_absolute() { link } else { target.parent().map(|d| d.join(&link)).unwrap_or_default() };
                if resolved == source { return Ok(()); }
            }
            return Err(AppError::Conflict(format!(
                "{}{} is a symlink pointing somewhere else", platform.path, skill_id
            )));
        }
        return Err(AppError::Conflict(format!(
            "{}{} already exists as a real file/directory", platform.path, skill_id
        )));
    }

    symlink(&source, &target)?;
    Ok(())
}

#[tauri::command]
pub fn remove_route(skill_id: String, platform_id: String) -> Result<()> {
    let platform = platform_by_id(&platform_id)
        .ok_or_else(|| AppError::PlatformNotFound(platform_id.clone()))?;
    if platform.is_hub { return Err(AppError::HubRoute); }

    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    let source = central.join(&skill_id);
    let platform_root = platform_dir(&platform_id).ok_or(AppError::NoHomeDir)?;
    let target = platform_root.join(&skill_id);

    let Ok(meta) = fs::symlink_metadata(&target) else {
        return Ok(()); // Nothing to do.
    };

    if !meta.file_type().is_symlink() {
        return Err(AppError::Conflict(format!(
            "{}{} is a real file/directory — refusing to delete", platform.path, skill_id
        )));
    }
    if let Ok(link) = fs::read_link(&target) {
        let resolved = if link.is_absolute() { link } else { target.parent().map(|d| d.join(&link)).unwrap_or_default() };
        if resolved != source {
            return Err(AppError::Conflict(format!(
                "{}{} links elsewhere — refusing to delete", platform.path, skill_id
            )));
        }
    }
    fs::remove_file(&target)?;
    Ok(())
}
