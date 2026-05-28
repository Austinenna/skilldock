use crate::error::{AppError, Result};
use crate::platforms::{platform_by_id, platform_dir};
use crate::skills::{existing_central_skill_paths, link_points_to_path, validate_skill_id};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

fn platform_skill_paths(platform_id: &str, skill_id: &str) -> Result<(PathBuf, PathBuf)> {
    validate_skill_id(skill_id)?;
    let platform_root = platform_dir(platform_id).ok_or(AppError::NoHomeDir)?;
    let target = platform_root.join(skill_id);
    Ok((platform_root, target))
}

#[tauri::command]
pub fn add_route(skill_id: String, platform_id: String) -> Result<()> {
    validate_skill_id(&skill_id)?;
    let platform = platform_by_id(&platform_id)
        .ok_or_else(|| AppError::PlatformNotFound(platform_id.clone()))?;
    if platform.is_hub {
        return Err(AppError::HubRoute);
    }

    let (source, canonical_source) = existing_central_skill_paths(&skill_id)?;

    let (platform_root, target) = platform_skill_paths(&platform_id, &skill_id)?;
    fs::create_dir_all(&platform_root)?;

    // If something already exists at the target, decide carefully.
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.file_type().is_symlink() {
            // Idempotent: same symlink to our central is fine.
            if let Ok(link) = fs::read_link(&target) {
                if link_points_to_path(link, &target, &source, &canonical_source) {
                    return Ok(());
                }
            }
            return Err(AppError::Conflict(format!(
                "{}{} is a symlink pointing somewhere else",
                platform.path, skill_id
            )));
        }
        return Err(AppError::Conflict(format!(
            "{}{} already exists as a real file/directory",
            platform.path, skill_id
        )));
    }

    symlink(&source, &target)?;
    Ok(())
}

#[tauri::command]
pub fn remove_route(skill_id: String, platform_id: String) -> Result<()> {
    validate_skill_id(&skill_id)?;
    let platform = platform_by_id(&platform_id)
        .ok_or_else(|| AppError::PlatformNotFound(platform_id.clone()))?;
    if platform.is_hub {
        return Err(AppError::HubRoute);
    }

    let (source, canonical_source) = existing_central_skill_paths(&skill_id)?;
    let (_, target) = platform_skill_paths(&platform_id, &skill_id)?;

    let Ok(meta) = fs::symlink_metadata(&target) else {
        return Ok(()); // Nothing to do.
    };

    if !meta.file_type().is_symlink() {
        return Err(AppError::Conflict(format!(
            "{}{} is a real file/directory — refusing to delete",
            platform.path, skill_id
        )));
    }
    if let Ok(link) = fs::read_link(&target) {
        if !link_points_to_path(link, &target, &source, &canonical_source) {
            return Err(AppError::Conflict(format!(
                "{}{} links elsewhere — refusing to delete",
                platform.path, skill_id
            )));
        }
    }
    fs::remove_file(&target)?;
    Ok(())
}
