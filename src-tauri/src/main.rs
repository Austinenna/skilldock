#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod ai;
mod config;
mod error;
mod platforms;
mod repository_init;
mod routes;
mod skills;
mod watcher;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let skill_watcher = watcher::build(app.handle().clone())?;
            app.manage(skill_watcher);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            platforms::list_platforms,
            repository_init::preview_repository_init,
            repository_init::run_repository_init,
            skills::scan_skills,
            skills::get_skill_detail,
            skills::import_skill,
            skills::delete_skill,
            routes::add_route,
            routes::remove_route,
            ai::test_ai_config,
            ai::generate_summary,
            config::get_config,
            config::update_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SkillLoom");
}
