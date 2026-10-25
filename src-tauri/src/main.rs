#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod config;
mod error;
mod skills;
mod watcher;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            if let Some(skill_watcher) = watcher::build(app.handle().clone())? {
                app.manage(skill_watcher);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            skills::get_overview,
            skills::get_skill_detail,
            skills::new_skill,
            skills::publish_skill,
            skills::set_skill_agents,
            skills::unpublish_skill,
            skills::delete_skill,
            skills::run_sync,
            skills::run_doctor,
            config::get_config,
            config::update_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SkillDock");
}
