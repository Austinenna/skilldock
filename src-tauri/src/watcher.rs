use crate::config;
use crate::error::{AppError, Result};
use crate::platforms::{central_dir, expand_path, PLATFORMS};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct SkillWatcher {
    _watcher: Mutex<RecommendedWatcher>,
}

fn watch_existing_dir(watcher: &mut RecommendedWatcher, path: &std::path::Path) -> Result<()> {
    if path.exists() {
        watcher.watch(path, RecursiveMode::Recursive)?;
    }
    Ok(())
}

pub fn build(app: AppHandle) -> Result<SkillWatcher> {
    let central = central_dir().ok_or(AppError::NoHomeDir)?;
    fs::create_dir_all(&central)?;

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default(),
    )?;

    watch_existing_dir(&mut watcher, &central)?;

    let cfg = config::get_config(app.clone()).unwrap_or_default();
    for platform in PLATFORMS
        .iter()
        .filter(|platform| !platform.is_hub && !cfg.hidden_platforms.contains(&platform.id))
    {
        let Some(path) = expand_path(&platform.path) else {
            continue;
        };
        watch_existing_dir(&mut watcher, &path)?;
    }

    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            if event.is_err() {
                continue;
            }

            while rx.recv_timeout(Duration::from_millis(350)).is_ok() {}
            let _ = app.emit("skills-changed", ());
        }
    });

    Ok(SkillWatcher {
        _watcher: Mutex::new(watcher),
    })
}
