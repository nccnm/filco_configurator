// Generated from src-tauri/src/view.rs by the `bindings_are_current` test. Do not edit.
// Regenerate: $env:UPDATE_BINDINGS=1; cargo test -p keyboard-layer-editor bindings

export type UiLang = "vi" | "en";

export type Phase = "no_device" | "unsupported" | "connected" | "ready" | "read_failed";

export type KeyView = { id: string, index: number, x: number, y: number, w: number, h: number, legend: string, m_key: boolean, unverified: boolean, note: string | null, };

export type LayerInfoView = { label: string, led: string, led_name: string, };

export type ModelView = { id: string, name: string, vendor: string, support: string, layers: Array<LayerInfoView>, keys: Array<KeyView>, 
/**
 * Size of the layout in key units.
 */
width: number, height: number, editable_types: Array<number>, notes: Array<string>, };

export type SlotView = { 
/**
 * `"02 00 a4 00"`.
 */
hex: string, 
/**
 * `empty`, `key`, `mouse`, `consumer`, `system`, `macro`, `other`.
 */
kind: string, 
/**
 * Keycap text; empty for an empty slot (the UI shows the printed legend faintly).
 */
short: string, long: string, 
/**
 * Differs from what the keyboard holds.
 */
changed: boolean, 
/**
 * The editor can produce this kind (types 0-4); others are shown and kept but not edited.
 */
editable: boolean, };

export type DeviceView = { path: string, name: string, vendor_id: number, product_id: number, version: number, model_id: string | null, };

export type ChangeView = { layer: number, index: number, 
/**
 * Key id in the layout, `None` for a slot without a physical key.
 */
key: string | null, legend: string | null, old: SlotView, new: SlotView, };

export type LinkedLayerView = { layer: number, profile: string, };

export type AssistView = { running: Array<string>, 
/**
 * The process list could not be read: writing is blocked.
 */
probe_error: string | null, 
/**
 * Layers FILCO Assist would rebuild from a profile.
 */
linked: Array<LinkedLayerView>, config_error: string | null, };

export type JournalLayerView = { layer: number, 
/**
 * `original`, `target` or `unknown`.
 */
state: string, };

export type JournalView = { started: string, backup: string, layers: Array<JournalLayerView>, };

export type PlanView = { layers: Array<number>, changes: Array<ChangeView>, backup: string, 
/**
 * Layers of the plan that FILCO Assist links to a profile.
 */
linked: Array<LinkedLayerView>, experimental: boolean, };

export type OutcomeView = { layer: number, 
/**
 * `written`, `not_written`, `restored`, `restore_failed`, `unknown`.
 */
outcome: string, };

export type ReportView = { succeeded: boolean, consistent: boolean, outcomes: Array<OutcomeView>, error: string | null, backup: string, journal_kept: boolean, };

export type ErrorView = { 
/**
 * Stable code the UI translates, e.g. `assist_running`, `device_changed`, `read_failed`.
 */
code: string, 
/**
 * Technical detail (English), shown under the translated text.
 */
detail: string, };

export type Snapshot = { phase: Phase, lang: UiLang, devices: Array<DeviceView>, device: DeviceView | null, model_id: string | null, 
/**
 * `layers[layer][i]` is the slot of `model.keys[i]`.
 */
layers: Array<Array<SlotView>>, changes: Array<ChangeView>, can_undo: boolean, can_redo: boolean, assist: AssistView, journal: JournalView | null, plan: PlanView | null, report: ReportView | null, error: ErrorView | null, simulated: boolean, };

export type BackupView = { name: string, path: string, created: string, reason: string | null, problem: string | null, };

export type ProgressView = { layer: number, 
/**
 * `read`, `write` or `verify`.
 */
phase: string, done: number, total: number, 
/**
 * Step of the whole operation, e.g. 3 of 8 page sessions.
 */
step: number, steps: number, };

export type KeyUsageView = { code: number, legend: string, name: string, group: string, };

export type ConsumerUsageView = { code: number, legend: string, name: string, filco_assist: boolean, };

export type SystemUsageView = { code: number, legend: string, name: string, };

export type Catalog = { keys: Array<KeyUsageView>, consumer: Array<ConsumerUsageView>, system: Array<SystemUsageView>, };

export type Action = { "type": "empty" } | { "type": "key", mods: number, usage: number, } | { "type": "mouse_click", buttons: number, } | { "type": "mouse_scroll", amount: number, } | { "type": "mouse_move", dx: number, dy: number, } | { "type": "consumer", usage: number, } | { "type": "system", usage: number, } | { "type": "raw", hex: string, };
