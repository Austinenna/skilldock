use crate::error::{AppError, Result};
use crate::platforms::{central_dir, platform_by_id, platform_dir};
use crate::skills::{link_points_to_path, validate_skill_id};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn existing_skill_paths_in_root(central_root: &Path, skill_id: &str) -> Result<(PathBuf, PathBuf)> {
    validate_skill_id(skill_id)?;
    let source = central_root.join(skill_id);
    if !source.exists() {
        return Err(AppError::SkillNotFound(skill_id.to_string()));
    }

    let canonical_root = central_root.canonicalize()?;
    let canonical_source = source.canonicalize()?;
    if !canonical_source.starts_with(&canonical_root) {
        return Err(AppError::Conflict(format!(
            "skill path for '{}' resolves outside central",
            skill_id
        )));
    }

    Ok((source, canonical_source))
}

fn platform_skill_paths(platform_id: &str, skill_id: &str) -> Result<(PathBuf, PathBuf)> {
    validate_skill_id(skill_id)?;
    let platform_root = platform_dir(platform_id).ok_or(AppError::NoHomeDir)?;
    let target = platform_root.join(skill_id);
    Ok((platform_root, target))
}

fn add_route_between_roots(
    skill_id: &str,
    central_root: &Path,
    platform_root: &Path,
    platform_path_label: &str,
) -> Result<()> {
    let (source, canonical_source) = existing_skill_paths_in_root(central_root, skill_id)?;

    fs::create_dir_all(platform_root)?;
    let target = platform_root.join(skill_id);

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
                platform_path_label, skill_id
            )));
        }
        return Err(AppError::Conflict(format!(
            "{}{} already exists as a real file/directory",
            platform_path_label, skill_id
        )));
    }

    symlink(&source, &target)?;
    Ok(())
}

fn remove_route_between_roots(
    skill_id: &str,
    central_root: &Path,
    platform_root: &Path,
    platform_path_label: &str,
) -> Result<()> {
    let (source, canonical_source) = existing_skill_paths_in_root(central_root, skill_id)?;
    let target = platform_root.join(skill_id);

    let Ok(meta) = fs::symlink_metadata(&target) else {
        return Ok(()); // Nothing to do.
    };

    if !meta.file_type().is_symlink() {
        return Err(AppError::Conflict(format!(
            "{}{} is a real file/directory — refusing to delete",
            platform_path_label, skill_id
        )));
    }

    let link = fs::read_link(&target).map_err(|_| {
        AppError::Conflict(format!(
            "{}{} is a symlink, but SkillLoom could not read its target",
            platform_path_label, skill_id
        ))
    })?;
    if !link_points_to_path(link, &target, &source, &canonical_source) {
        return Err(AppError::Conflict(format!(
            "{}{} links elsewhere — refusing to delete",
            platform_path_label, skill_id
        )));
    }

    fs::remove_file(&target)?;
    Ok(())
}

#[tauri::command]
pub fn add_route(skill_id: String, platform_id: String) -> Result<()> {
    validate_skill_id(&skill_id)?;
    let platform = platform_by_id(&platform_id)
        .ok_or_else(|| AppError::PlatformNotFound(platform_id.clone()))?;
    if platform.is_hub {
        return Err(AppError::HubRoute);
    }

    let central_root = central_dir().ok_or(AppError::NoHomeDir)?;
    let (platform_root, _) = platform_skill_paths(&platform_id, &skill_id)?;
    add_route_between_roots(&skill_id, &central_root, &platform_root, &platform.path)
}

#[tauri::command]
pub fn remove_route(skill_id: String, platform_id: String) -> Result<()> {
    validate_skill_id(&skill_id)?;
    let platform = platform_by_id(&platform_id)
        .ok_or_else(|| AppError::PlatformNotFound(platform_id.clone()))?;
    if platform.is_hub {
        return Err(AppError::HubRoute);
    }

    let central_root = central_dir().ok_or(AppError::NoHomeDir)?;
    let (platform_root, _) = platform_skill_paths(&platform_id, &skill_id)?;
    remove_route_between_roots(&skill_id, &central_root, &platform_root, &platform.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SKILL_ID: &str = "demo-skill";

    struct RouteFixture {
        root: PathBuf,
        central: PathBuf,
        platform: PathBuf,
    }

    impl RouteFixture {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos();
            let root = env::temp_dir().join(format!(
                "skillloom-route-test-{}-{}-{}",
                std::process::id(),
                nonce,
                name
            ));
            let central = root.join("central");
            let platform = root.join("platform");
            fs::create_dir_all(central.join(SKILL_ID)).expect("create central skill");
            fs::create_dir_all(&platform).expect("create platform root");

            Self {
                root,
                central,
                platform,
            }
        }

        fn source(&self) -> PathBuf {
            self.central.join(SKILL_ID)
        }

        fn target(&self) -> PathBuf {
            self.platform.join(SKILL_ID)
        }
    }

    impl Drop for RouteFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn assert_conflict(result: Result<()>) {
        match result {
            Err(AppError::Conflict(_)) => {}
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn add_route_is_idempotent_when_symlink_points_to_central() {
        let fixture = RouteFixture::new("idempotent");
        symlink(fixture.source(), fixture.target()).expect("create existing route");

        add_route_between_roots(SKILL_ID, &fixture.central, &fixture.platform, "platform/")
            .expect("idempotent add should succeed");

        assert_eq!(
            fs::read_link(fixture.target()).expect("read route link"),
            fixture.source()
        );
    }

    #[test]
    fn add_route_rejects_real_target_directory() {
        let fixture = RouteFixture::new("real-dir-conflict");
        fs::create_dir(fixture.target()).expect("create conflicting directory");

        assert_conflict(add_route_between_roots(
            SKILL_ID,
            &fixture.central,
            &fixture.platform,
            "platform/",
        ));
        assert!(fixture.target().is_dir());
    }

    #[test]
    fn add_route_rejects_symlink_pointing_elsewhere() {
        let fixture = RouteFixture::new("other-link-conflict");
        let other = fixture.root.join("other-skill");
        fs::create_dir_all(&other).expect("create other target");
        symlink(&other, fixture.target()).expect("create conflicting symlink");

        assert_conflict(add_route_between_roots(
            SKILL_ID,
            &fixture.central,
            &fixture.platform,
            "platform/",
        ));
        assert_eq!(
            fs::read_link(fixture.target()).expect("read conflicting link"),
            other
        );
    }

    #[test]
    fn remove_route_rejects_real_target_directory() {
        let fixture = RouteFixture::new("remove-real-dir");
        fs::create_dir(fixture.target()).expect("create real route directory");

        assert_conflict(remove_route_between_roots(
            SKILL_ID,
            &fixture.central,
            &fixture.platform,
            "platform/",
        ));
        assert!(fixture.target().is_dir());
    }

    #[test]
    fn remove_route_deletes_symlink_to_central() {
        let fixture = RouteFixture::new("remove-symlink");
        symlink(fixture.source(), fixture.target()).expect("create route");

        remove_route_between_roots(SKILL_ID, &fixture.central, &fixture.platform, "platform/")
            .expect("remove route should succeed");

        assert!(!fixture.target().exists());
    }
}
