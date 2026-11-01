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

/// The repository's `firmware/` (stock HEX and updater), used when no bundled copy is found (dev runs, E2E).
fn dev_firmware_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../firmware")
}

/// Window height on first open (logical px), so the inspector under the keyboard shows without resizing. The
/// config keeps 1280 x 800, which the E2E screenshots use; a real build grows the window up to this height,
/// within the monitor's work area, and centres it there.
#[cfg(not(feature = "e2e-sim"))]
const OPEN_HEIGHT: f64 = 1000.0;

#[cfg(not(feature = "e2e-sim"))]
fn fit_window(tauri_app: &tauri::App) {
    use tauri::Manager as _;
    let Some(w) = tauri_app.get_webview_window("main") else {
        return;
    };
    let (Ok(Some(monitor)), Ok(size)) = (w.current_monitor(), w.inner_size()) else {
        return;
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let area = work.size.to_logical::<f64>(scale);
    let top = work.position.to_logical::<f64>(scale);
    let inner = size.to_logical::<f64>(scale);
    let outer = w.outer_size().unwrap_or(size).to_logical::<f64>(scale);
    // Title bar and frame, so the whole window fits above the taskbar.
    let frame_w = (outer.width - inner.width).max(0.0);
    let frame_h = (outer.height - inner.height).max(0.0);
    let height = OPEN_HEIGHT.min(area.height - frame_h).max(inner.height);
    if height > inner.height {
        let _ = w.set_size(tauri::LogicalSize::new(inner.width, height));
        // Centred in the work area: centring on the whole screen would put the bottom under the taskbar.
        let _ = w.set_position(tauri::LogicalPosition::new(
            top.x + ((area.width - inner.width - frame_w) / 2.0).max(0.0),
            top.y + ((area.height - height - frame_h) / 2.0).max(0.0),
        ));
    }
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
        firmware: Some(dev_firmware_dir()),
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
            firmware: None,
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
    let app = App::new(backend, &paths, simulated);
    #[cfg(feature = "e2e-sim")]
    let app = {
        let mut app = app;
        app.set_launcher(commands::e2e::launcher(sim.clone()));
        app
    };
    let app = Arc::new(Mutex::new(app));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(State {
            app: Arc::clone(&app),
            #[cfg(feature = "e2e-sim")]
            sim,
            #[cfg(feature = "e2e-sim")]
            started,
        })
        .setup(move |tauri_app| {
            // The stock firmware and updater ship as bundle resources; a dev run uses the repository's copy.
            #[cfg(not(feature = "e2e-sim"))]
            {
                use tauri::Manager as _;
                let dir = tauri_app
                    .path()
                    .resource_dir()
                    .map(|d| d.join("firmware"))
                    .ok()
                    .filter(|d| d.join(filco_firmware::STOCK_HEX_NAME).is_file())
                    .unwrap_or_else(dev_firmware_dir);
                app.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_firmware_dir(&dir);
            }
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
            #[cfg(not(feature = "e2e-sim"))]
            fit_window(tauri_app);
            let _ = tauri_app.handle().emit("backend-ready", ());
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
