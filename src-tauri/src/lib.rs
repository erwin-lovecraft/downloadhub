mod commands;
mod state;

use state::AppState;

fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::from_env())
        .invoke_handler(tauri::generate_handler![
            commands::auth::auth_login,
            commands::auth::auth_logout,
            commands::auth::auth_status,
            commands::youtube::search_videos,
            commands::video::get_video_formats,
            commands::queue::add_to_queue,
            commands::queue::list_queue,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Dev convenience: load GOOGLE_OAUTH_CLIENT_ID/SECRET from a gitignored
    // .env file if present. No-op (and not an error) if the file is missing.
    let _ = dotenvy::dotenv();

    configure(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_builds_with_registered_commands() {
        let app = configure(tauri::test::mock_builder())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("app should build with auth commands registered");
        drop(app);
    }
}
