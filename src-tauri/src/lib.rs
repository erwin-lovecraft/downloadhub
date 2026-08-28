mod commands;
mod state;

use state::AppState;

fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::from_env())
        .invoke_handler(tauri::generate_handler![
            commands::youtube::search_videos,
            commands::youtube::list_playlist_items,
            commands::video::get_video_formats,
            commands::queue::add_to_queue,
            commands::queue::list_queue,
            commands::queue::remove_from_queue,
            commands::queue::clear_queue,
            commands::queue::set_queue_entry_format,
            commands::queue::set_queue_entries_quality,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::download::download_all,
            commands::download::stop_download_all,
            commands::playlist::import_playlist_to_queue,
            commands::cookies::check_ytdlp_cookies,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::mcp::mcp_server_path,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Dev convenience: load YOUTUBE_API_KEY from a gitignored .env file if
    // present. No-op (and not an error) if the file is missing.
    let _ = dotenvy::dotenv();

    // One-time upgrade for a settings file still holding pasted cookie text,
    // from before cookies became a path to the user's own file. Best-effort
    // and blocking: it's one small file, and it has to finish before any
    // command can read settings.
    if let Some(app_data_dir) = downloadhub_core::paths::app_data_dir() {
        let settings_path = downloadhub_core::paths::settings_path(&app_data_dir);
        if let Err(e) = tauri::async_runtime::block_on(
            downloadhub_core::settings::migrate_pasted_cookies(&settings_path),
        ) {
            eprintln!("failed to migrate pasted yt-dlp cookies to a file: {e}");
        }
    }

    // The updater plugin needs the `plugins.updater` config (pubkey +
    // endpoints), so it lives here rather than in `configure`, which is also
    // driven by tests against a mock context that has no such config.
    configure(tauri::Builder::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            .expect("app should build with registered commands");
        drop(app);
    }
}
