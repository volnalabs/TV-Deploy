//! TV Deploy desktop shell. The React frontend can only call the typed commands below; it
//! never starts processes or touches arbitrary paths itself.

mod api;
mod settings;
mod toolchain;

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;
use tvdeploy_core::samsung::runner::CancelToken;

pub struct AppState {
    pub data_dir: std::path::PathBuf,
    pub settings: Mutex<settings::Settings>,
    pub ops: Mutex<HashMap<String, CancelToken>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&dir).ok();
            let settings = settings::load(&dir);
            app.manage(AppState {
                data_dir: dir,
                settings: Mutex::new(settings),
                ops: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::toolchain_status,
            api::find_existing_sdk,
            api::set_sdk_path,
            api::scan_tvs,
            api::connect_tv,
            api::disconnect_tv,
            api::analyze_package,
            api::cancel_operation,
            api::get_settings,
            api::save_preferences,
            api::remove_device,
            api::rename_device,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TV Deploy");
}
