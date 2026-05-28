#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod error;
mod platforms;
mod routes;
mod skills;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            platforms::list_platforms,
            skills::scan_skills,
            skills::import_skill,
            skills::delete_skill,
            routes::add_route,
            routes::remove_route,
            config::get_config,
            config::update_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SkillLoom");
}
