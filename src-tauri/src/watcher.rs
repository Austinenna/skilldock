use crate::error::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct SkillWatcher {
    _watcher: Mutex<RecommendedWatcher>,
}

/// Watch only the skl root, `dev/` and `released/` top levels (and thus
/// skills.toml). Edits deep inside a skill are picked up by manual Refresh;
/// recursive watching would wake on every node_modules change.
pub fn build(app: AppHandle) -> Result<Option<SkillWatcher>> {
    let Ok(root) = skilldock_core::resolve_root() else {
        return Ok(None);
    };
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default(),
    )?;
    for dir in [root.path.clone(), root.dev(), root.released()] {
        if dir.is_dir() {
            watcher.watch(&dir, RecursiveMode::NonRecursive)?;
        }
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

    Ok(Some(SkillWatcher {
        _watcher: Mutex::new(watcher),
    }))
}
