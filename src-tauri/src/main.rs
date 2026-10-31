#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod view;
#[cfg(all(windows, not(feature = "e2e-sim")))]
mod win_backend;

use std::sync::{Arc, Mutex};

use filco_service::backend::Backend;
use tauri::Emitter;

use crate::app::{App, Paths};

#[allow(missing_debug_implementations)]
pub struct State {
    pub app: Arc<Mutex<App>>,
    #[cfg(feature = "e2e-sim")]
    pub sim: filco_service::backend::SimBackend,
    /// When `main` started, for the startup measurement of the E2E build.
    #[cfg(feature = "e2e-sim")]
    pub started: std::time::SystemTime,
}

#[cfg(feature = "e2e-sim")]
fn backend() -> (Arc<dyn Backend>, filco_service::backend::SimBackend, Paths, bool) {
    let sim = commands::e2e::initial_sim();
    let root = std::env::var_os("KLE_E2E_DIR").map_or_else(
        || std::env::temp_dir().join("keyboard-layer-editor-e2e"),
        std::path::PathBuf::from,
    );
    let paths = Paths {
        backups: root.join("backups"),
        state: root.clone(),
        assist_config: None,
    };
    (Arc::new(sim.clone()), sim, paths, true)
}

#[cfg(not(feature = "e2e-sim"))]
fn backend() -> (Arc<dyn Backend>, Paths, bool) {
    let paths = Paths::default_for_user().unwrap_or_else(|| {
        let tmp = std::env::temp_dir().join("keyboard-layer-editor");
        Paths {
            backups: tmp.join("backups"),
            state: tmp,
            assist_config: None,
        }
    });
    (Arc::new(win_backend::WindowsBackend), paths, false)
}

fn main() {
    #[cfg(feature = "e2e-sim")]
    let started = std::time::SystemTime::now();
    #[cfg(feature = "e2e-sim")]
    let (backend, sim, paths, simulated) = backend();
    #[cfg(not(feature = "e2e-sim"))]
    let (backend, paths, simulated) = backend();
    let app = Arc::new(Mutex::new(App::new(backend, &paths, simulated)));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(State {
            app: Arc::clone(&app),
            #[cfg(feature = "e2e-sim")]
            sim,
            #[cfg(feature = "e2e-sim")]
            started,
        })
        .setup(|tauri_app| {
            // Plug and unplug: tell the UI, which asks for a refresh.
            #[cfg(all(windows, not(feature = "e2e-sim")))]
            {
                use tauri::Manager as _;
                let handle = tauri_app.handle().clone();
                match filco_hid_windows::HidWatcher::new(move || {
                    let _ = handle.emit("devices-changed", ());
                }) {
                    Ok(w) => {
                        tauri_app.manage(Mutex::new(Some(w)));
                    }
                    Err(e) => eprintln!("device notifications unavailable: {e}"),
                }
            }
            let _ = tauri_app.handle().emit("backend-ready", ());
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
