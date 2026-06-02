pub mod profile_store;
pub mod github;
pub mod local_packages;
pub mod sync_orchestrator;
pub mod update_check;
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,controller_pack_app_lib=debug".into()),
        )
        .try_init()
        .ok();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init());

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_process::init())
            .plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                update_check::spawn_background_checker(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_profile,
            commands::update_profile,
            commands::detect_pack_dir,
            commands::looks_like_controller_pack,
            commands::run_sync,
            commands::update_from_github,
            commands::apply_profile_to_pack,
            commands::import_plugin_lines,
            commands::check_updates,
            commands::check_installer_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
