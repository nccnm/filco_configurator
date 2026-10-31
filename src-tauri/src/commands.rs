//! Thin Tauri commands: run the [`App`](crate::app::App) method on a blocking thread (device I/O never runs on
//! the UI thread), stream progress as `progress` events, return the new snapshot.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::app::App;
use crate::view::{Action, BackupView, Catalog, ModelView, ProgressView, Snapshot, UiLang};

type Res<T> = Result<T, String>;

async fn run<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    state: &State<'_, crate::State>,
    f: impl FnOnce(&mut App, &mut dyn FnMut(ProgressView)) -> T + Send + 'static,
) -> Res<T> {
    let shared = std::sync::Arc::clone(&state.app);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut emit = |p: ProgressView| {
            let _ = handle.emit("progress", p);
        };
        f(&mut guard, &mut emit)
    })
    .await
    .map_err(|e| e.to_string())
}

macro_rules! snapshot_command {
    ($name:ident ( $($arg:ident : $ty:ty),* ) => |$a:ident, $p:pat_param| $body:expr) => {
        #[tauri::command]
        pub async fn $name<R: Runtime>(
            app: AppHandle<R>,
            state: State<'_, crate::State>,
            $($arg: $ty),*
        ) -> Res<Snapshot> {
            run(&app, &state, move |$a, $p| {
                $body;
                $a.snapshot()
            })
            .await
        }
    };
}

snapshot_command!(refresh() => |a, p| a.refresh(p));
snapshot_command!(connect(path: String) => |a, p| a.connect(&path, p));
snapshot_command!(read() => |a, p| a.read(p));
snapshot_command!(set_lang(lang: UiLang) => |a, _| a.set_lang(lang));
snapshot_command!(assign(layers: Vec<usize>, index: u8, action: Action) => |a, _| a.assign(&layers, index, &action));
snapshot_command!(copy_layer(from: usize, to: usize) => |a, _| a.copy_layer(from, to));
snapshot_command!(undo() => |a, _| a.undo());
snapshot_command!(redo() => |a, _| a.redo());
snapshot_command!(revert() => |a, _| a.revert());
snapshot_command!(prepare_write() => |a, p| a.prepare_write(p));
snapshot_command!(cancel_write() => |a, _| a.cancel_write());
snapshot_command!(execute_write() => |a, p| a.execute_write(p));
snapshot_command!(dismiss_report() => |a, _| a.dismiss_report());
snapshot_command!(dismiss_journal() => |a, _| a.dismiss_journal());
snapshot_command!(restore_from_journal() => |a, _| a.restore_from_journal());
snapshot_command!(load_backup(path: String) => |a, _| a.load_backup(&PathBuf::from(path)));
snapshot_command!(backup_now() => |a, p| { a.backup_now(p); });

#[tauri::command]
pub async fn snapshot<R: Runtime>(app: AppHandle<R>, state: State<'_, crate::State>) -> Res<Snapshot> {
    run(&app, &state, |a, _| a.snapshot()).await
}

#[tauri::command]
pub async fn model<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, crate::State>,
    id: String,
) -> Res<Option<ModelView>> {
    run(&app, &state, move |a, _| a.model_view(&id)).await
}

#[tauri::command]
pub fn catalog() -> Catalog {
    Catalog::new()
}

#[tauri::command]
pub async fn backups<R: Runtime>(app: AppHandle<R>, state: State<'_, crate::State>) -> Res<Vec<BackupView>> {
    run(&app, &state, |a, _| a.backups()).await
}

#[tauri::command]
pub async fn open_backup_folder<R: Runtime>(app: AppHandle<R>, state: State<'_, crate::State>) -> Res<()> {
    let dir = run(&app, &state, |a, _| a.backup_dir()).await?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe")
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Asks for a file name, then exports the edited pages. `false` when the user cancelled.
#[tauri::command]
pub async fn export_file<R: Runtime>(app: AppHandle<R>, state: State<'_, crate::State>) -> Res<Snapshot> {
    let picked = app
        .dialog()
        .file()
        .add_filter("FILCO layers", &["json"])
        .set_file_name(format!("layers.{}", filco_core::export::EXTENSION))
        .blocking_save_file();
    let path = picked.and_then(|p| p.into_path().ok());
    run(&app, &state, move |a, _| {
        if let Some(p) = path {
            a.export_file(&p);
        }
        a.snapshot()
    })
    .await
}

#[tauri::command]
pub async fn import_file<R: Runtime>(app: AppHandle<R>, state: State<'_, crate::State>) -> Res<Snapshot> {
    let picked = app
        .dialog()
        .file()
        .add_filter("FILCO layers", &["json"])
        .blocking_pick_file();
    let path = picked.and_then(|p| p.into_path().ok());
    run(&app, &state, move |a, _| {
        if let Some(p) = path {
            a.import_file(&p);
        }
        a.snapshot()
    })
    .await
}

/// Picks a backup folder anywhere (e.g. one made by `fnmap.py`).
#[tauri::command]
pub async fn load_backup_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, crate::State>,
) -> Res<Snapshot> {
    let picked = app.dialog().file().blocking_pick_folder();
    let path = picked.and_then(|p| p.into_path().ok());
    run(&app, &state, move |a, _| {
        if let Some(p) = path {
            a.load_backup(&p);
        }
        a.snapshot()
    })
    .await
}

/// Controls for the simulated keyboard of the `e2e-sim` build.
#[cfg(feature = "e2e-sim")]
pub mod e2e {
    use filco_core::Page;
    use filco_service::backend::SimBackend;
    use filco_sim::{Fault, SimKeyboard};

    use super::{AppHandle, Res, Runtime, Snapshot, State, run};

    const FIXTURE: [&[u8]; 4] = [
        include_bytes!("../../tests/fixtures/pages/2026-10-31_Layer4Snip/layer0.bin"),
        include_bytes!("../../tests/fixtures/pages/2026-10-31_Layer4Snip/layer1.bin"),
        include_bytes!("../../tests/fixtures/pages/2026-10-31_Layer4Snip/layer2.bin"),
        include_bytes!("../../tests/fixtures/pages/2026-10-31_Layer4Snip/layer3.bin"),
    ];

    pub fn pages() -> Vec<Page> {
        FIXTURE
            .iter()
            .map(|b| Page::from_bytes(b).unwrap_or_else(|_| Page::empty()))
            .collect()
    }

    pub fn initial_sim() -> SimBackend {
        let mut sim = SimBackend::new(SimKeyboard::new(pages()));
        let pace = std::env::var("KLE_E2E_PACE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        sim.pace = Some(std::time::Duration::from_millis(pace));
        if std::env::var_os("KLE_E2E_UNPLUGGED").is_some() {
            sim.with(SimKeyboard::unplug);
        }
        sim
    }

    /// Startup measurement: with `KLE_E2E_STARTUP_FILE` set, writes when `main` started and when the page reached
    /// its marks (epoch ms), then quits. Without it, does nothing.
    #[tauri::command]
    #[allow(clippy::needless_pass_by_value)] // Tauri hands commands owned arguments.
    pub fn e2e_startup<R: Runtime>(
        app: AppHandle<R>,
        state: State<'_, crate::State>,
        origin: f64,
        ready: f64,
        interactive: f64,
    ) -> Res<()> {
        let Some(file) = std::env::var_os("KLE_E2E_STARTUP_FILE") else {
            return Ok(());
        };
        let started = state
            .started
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64()
            * 1000.0;
        let json = format!(
            "{{\"main_epoch_ms\":{started:.1},\"page_origin_epoch_ms\":{origin:.1},\"ready_ms\":{ready:.1},\"interactive_ms\":{interactive:.1}}}
"
        );
        std::fs::write(file, json).map_err(|e| e.to_string())?;
        app.exit(0);
        Ok(())
    }

    /// `op`: `unplug`, `replug`, `reset`, `assist_on`, `assist_off`, `fault:<json>`, `fault_next:<json>`.
    #[tauri::command]
    pub async fn e2e_sim<R: Runtime>(
        app: AppHandle<R>,
        state: State<'_, crate::State>,
        op: String,
    ) -> Res<Snapshot> {
        let sim = state.sim.clone();
        match op.as_str() {
            "unplug" => sim.with(SimKeyboard::unplug),
            "replug" => sim.with(SimKeyboard::reconnect),
            "reset" => sim.with(|k| {
                *k.flash_mut() = pages();
                k.reconnect();
            }),
            "assist_on" => sim.set_processes(&["FilcoAssistUI.exe", "FilcoAssistTray.exe"]),
            "assist_off" => sim.set_processes(&[]),
            other => {
                // `fault:` counts sessions from the start, `fault_next:` from the next session of that command.
                let (json, relative) = match (other.strip_prefix("fault:"), other.strip_prefix("fault_next:"))
                {
                    (Some(j), _) => (j, false),
                    (_, Some(j)) => (j, true),
                    _ => return Err("unknown op".into()),
                };
                let mut fault: Fault = serde_json::from_str(json).map_err(|e| e.to_string())?;
                sim.with(|k| {
                    if relative {
                        fault.nth += k.sessions(fault.cmd);
                    }
                    k.add_fault(fault);
                });
            }
        }
        run(&app, &state, |a, p| {
            a.refresh(p);
            a.snapshot()
        })
        .await
    }
}

#[cfg(not(feature = "e2e-sim"))]
pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        snapshot,
        refresh,
        connect,
        read,
        set_lang,
        assign,
        copy_layer,
        undo,
        redo,
        revert,
        prepare_write,
        cancel_write,
        execute_write,
        dismiss_report,
        dismiss_journal,
        restore_from_journal,
        load_backup,
        backup_now,
        model,
        catalog,
        backups,
        open_backup_folder,
        export_file,
        import_file,
        load_backup_folder
    ]
}

#[cfg(feature = "e2e-sim")]
pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        snapshot,
        refresh,
        connect,
        read,
        set_lang,
        assign,
        copy_layer,
        undo,
        redo,
        revert,
        prepare_write,
        cancel_write,
        execute_write,
        dismiss_report,
        dismiss_journal,
        restore_from_journal,
        load_backup,
        backup_now,
        model,
        catalog,
        backups,
        open_backup_folder,
        export_file,
        import_file,
        load_backup_folder,
        e2e::e2e_sim,
        e2e::e2e_startup
    ]
}
