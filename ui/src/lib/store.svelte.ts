// UI state. The keyboard state itself (pages, edits, undo, diff) lives in Rust and arrives as a Snapshot; this
// store only keeps what is purely visual: selection, open dialogs, progress, theme.
import type { Catalog, ModelView, ProgressView, Snapshot, UiLang } from "../bindings/ipc";
import { translate, type Key } from "./i18n";
import { ipc } from "./ipc";

export type Theme = "system" | "light" | "dark";
export type Busy = null | "refresh" | "read" | "prepare" | "write" | "backup" | "edit";
/** Dialogs not driven by the snapshot (the diff and report dialogs follow `snap.plan` / `snap.report`). */
export type Dialog = null | "backups";

class Store {
  snap = $state<Snapshot | null>(null);
  model = $state<ModelView | null>(null);
  catalog = $state<Catalog | null>(null);
  layer = $state(0);
  selected = $state<number | null>(null); // index into model.keys
  busy = $state<Busy>(null);
  progress = $state<ProgressView | null>(null);
  dialog = $state<Dialog>(null);
  theme = $state<Theme>("system");
  notice = $state<string | null>(null);
  ipcError = $state<string | null>(null);

  lang: UiLang = $derived(this.snap?.lang ?? "vi");

  t = (key: Key, vars: Record<string, string | number> = {}): string => translate(this.lang, key, vars);

  /** Runs a command that returns a snapshot; one at a time. */
  async run(kind: Exclude<Busy, null>, f: () => Promise<Snapshot>): Promise<void> {
    if (this.busy) return;
    this.busy = kind;
    this.progress = null;
    try {
      await this.apply(await f());
      this.ipcError = null;
    } catch (e) {
      this.ipcError = String(e);
    } finally {
      this.busy = null;
      this.progress = null;
    }
  }

  async apply(s: Snapshot): Promise<void> {
    if (s.model_id && s.model_id !== this.model?.id) {
      this.model = await ipc.model(s.model_id);
      this.selected = null;
    } else if (!s.model_id) {
      this.model = null;
      this.selected = null;
    }
    if (this.layer >= s.layers.length && s.layers.length > 0) this.layer = 0;
    this.snap = s;
  }

  /** Quiet refresh of FILCO Assist state and devices while idle (never while busy). */
  async poll(): Promise<void> {
    if (this.busy) return;
    try {
      const s = await ipc.snapshot();
      if (!this.busy) await this.apply(s);
    } catch (e) {
      this.ipcError = String(e);
    }
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
    localStorage.setItem("theme", theme);
    applyTheme(theme);
  }
}

export function applyTheme(theme: Theme): void {
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

export const store = new Store();
