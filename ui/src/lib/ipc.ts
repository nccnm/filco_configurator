// Typed wrappers around the Tauri commands of src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Action, BackupView, Catalog, ModelView, ProgressView, Snapshot, UiLang } from "../bindings/ipc";

export const ipc = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  refresh: () => invoke<Snapshot>("refresh"),
  connect: (path: string) => invoke<Snapshot>("connect", { path }),
  read: () => invoke<Snapshot>("read"),
  setLang: (lang: UiLang) => invoke<Snapshot>("set_lang", { lang }),
  assign: (layers: number[], index: number, action: Action) =>
    invoke<Snapshot>("assign", { layers, index, action }),
  copyLayer: (from: number, to: number) => invoke<Snapshot>("copy_layer", { from, to }),
  undo: () => invoke<Snapshot>("undo"),
  redo: () => invoke<Snapshot>("redo"),
  revert: () => invoke<Snapshot>("revert"),
  prepareWrite: () => invoke<Snapshot>("prepare_write"),
  cancelWrite: () => invoke<Snapshot>("cancel_write"),
  executeWrite: () => invoke<Snapshot>("execute_write"),
  dismissReport: () => invoke<Snapshot>("dismiss_report"),
  dismissJournal: () => invoke<Snapshot>("dismiss_journal"),
  restoreFromJournal: () => invoke<Snapshot>("restore_from_journal"),
  loadBackup: (path: string) => invoke<Snapshot>("load_backup", { path }),
  backupNow: () => invoke<Snapshot>("backup_now"),
  model: (id: string) => invoke<ModelView | null>("model", { id }),
  catalog: () => invoke<Catalog>("catalog"),
  backups: () => invoke<BackupView[]>("backups"),
  openBackupFolder: () => invoke<null>("open_backup_folder"),
  exportFile: () => invoke<Snapshot>("export_file"),
  importFile: () => invoke<Snapshot>("import_file"),
  loadBackupFolder: () => invoke<Snapshot>("load_backup_folder"),
  /** Only in the e2e-sim build. */
  e2eSim: (op: string) => invoke<Snapshot>("e2e_sim", { op }),
};

export function onProgress(f: (p: ProgressView) => void): Promise<UnlistenFn> {
  return listen<ProgressView>("progress", (e) => f(e.payload));
}

export function onDevicesChanged(f: () => void): Promise<UnlistenFn> {
  return listen("devices-changed", () => f());
}
