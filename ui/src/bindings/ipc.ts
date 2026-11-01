// Generated from src-tauri/src/view.rs by the `bindings_are_current` test. Do not edit.
// Regenerate: $env:UPDATE_BINDINGS=1; cargo test -p keyboard-layer-editor bindings

export type UiLang = "vi" | "en";

export type Phase = "no_device" | "unsupported" | "connected" | "ready" | "read_failed";

export type KeyView = { id: string, index: number, x: number, y: number, w: number, h: number, legend: string, 
/**
 * Shift character printed above `legend` (`{` above `[`), when the key's default usage has one.
 */
shifted: string | null, m_key: boolean, unverified: boolean, note: string | null, };

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
short: string, 
/**
 * Small line above `short` on the keycap: the modifiers of a long key combination, whose key alone is then
 * in `short` (so "Ctrl+Shift+Alt+E" stays readable).
 */
cap_sub: string | null, 
/**
 * Shift character drawn above `short` for a plain digit or symbol key (`{` above `[`).
 */
cap_shift: string | null, long: string, 
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

export type TapHoldView = { index: number, 
/**
 * Key id on this model, if the index has a key.
 */
key: string | null, 
/**
 * Keycap legend of the key.
 */
legend: string | null, 
/**
 * What a quick tap sends (a key legend), the keycap's main label.
 */
tap: string, 
/**
 * What holding does: a key legend, or `→ L3` when it turns on a layer. Drawn small above the tap.
 */
hold: string, 
/**
 * True when holding turns on a layer (the key is bold on that layer).
 */
hold_is_layer: boolean, 
/**
 * Layer index 0-3 that holding turns on, when `hold_is_layer`.
 */
hold_layer: number | null, term_ms: number, };

export type FirmwareEditView = { index: number, key: string | null, legend: string | null, 
/**
 * The code the change sets, as a legend; `None` means the Fn function was removed.
 */
code: string | null, };

export type FirmwareView = { name: string, tap_holds: Array<TapHoldView>, fn_edits: Array<FirmwareEditView>, keymap_edits: Array<FirmwareEditView>, };

export type FwCodeView = { code: number, 
/**
 * Keycap text; empty when `icon` is set.
 */
legend: string, 
/**
 * `lclick`, `rclick`, `mup`, `mdown`, `mleft`, `mright`, `wheelup`, `wheeldown`, `mute`, `voldown`, `volup`,
 * `menu`.
 */
icon: string | null, 
/**
 * Full name for the inspector and tooltips.
 */
name: string, };

export type FwHoldView = { "type": "key", code: number, } | { "type": "layer", layer: number, };

export type FwTapHoldView = { tap: number, hold: FwHoldView, term_ms: number, };

export type FwKeyView = { index: number, 
/**
 * Fn + key now; `None` when the key has no Fn function.
 */
fn_code: FwCodeView | null, fn_base: FwCodeView | null, fn_stock: FwCodeView | null, default: FwCodeView, default_base: FwCodeView, default_stock: FwCodeView, tap_hold: FwTapHoldView | null, 
/**
 * Keycap texts of the tap-hold: hold (`Ctrl`, `→ L3`) above, tap in the middle.
 */
tap_label: string | null, hold_label: string | null, 
/**
 * Differs from what the keyboard holds (not built and flashed yet).
 */
changed: boolean, 
/**
 * Differs from stock V72.
 */
custom: boolean, 
/**
 * `layout_specific`, `fn_key`, `tap_hold`.
 */
fn_locked: string | null, 
/**
 * `has_fn`, `fn_key`.
 */
tap_hold_locked: string | null, };

export type FwStepView = { 
/**
 * `image`, `keys`, `fn`, `layers`, `tap_hold`, `info`, `stack`, `updater`.
 */
id: string, ok: number, fail: number, };

export type FwBuildView = { name: string, running: boolean, steps: Array<FwStepView>, 
/**
 * Set when finished.
 */
passed: boolean | null, tag: number | null, dir: string | null, has_updater: boolean, 
/**
 * Failed report lines (when not passed).
 */
failures: Array<string>, 
/**
 * The whole report, for "show report".
 */
report: Array<string>, 
/**
 * The pipeline could not run (I/O, refused config).
 */
error: string | null, 
/**
 * The draft changed after this build: it can no longer be flashed from here.
 */
stale: boolean, };

export type FwFlashView = { name: string, tag: number, 
/**
 * `ready` (backup done, Assist closed), `waiting` (updater started, waiting for the keyboard to come back),
 * `done` (checked after flashing).
 */
step: string, backup: string | null, 
/**
 * The keyboard went away after the updater started (it is being flashed).
 */
saw_unplug: boolean, 
/**
 * After flashing: the firmware on the keyboard is the build.
 */
firmware_ok: boolean | null, 
/**
 * After flashing: name the keyboard reports (`None`: no info block).
 */
firmware_name: string | null, 
/**
 * After flashing: layers that differ from the backup.
 */
layers_changed: Array<number>, };

export type FwBuiltView = { name: string, dir: string, tag: number | null, passed: boolean, 
/**
 * Seconds since the epoch (a JSON number, well inside the safe integer range).
 */
modified: number, 
/**
 * This build is what the keyboard runs now.
 */
on_keyboard: boolean, };

export type FwView = { 
/**
 * The stock firmware shipped with the app could not be loaded: firmware mode is unavailable.
 */
stock_error: string | null, 
/**
 * Name and checksum of the firmware on the keyboard; `None` when it has no info block (stock V72 or an
 * older patched build).
 */
on_keyboard: string | null, on_keyboard_tag: number | null, keys: Array<FwKeyView>, 
/**
 * Keys that differ from the keyboard.
 */
changes: number, can_undo: boolean, can_redo: boolean, fn_slots_used: number, fn_slots: number, tap_key_used: number, tap_layer_used: number, 
/**
 * Index of the Fn key.
 */
fn_key: number, suggested_name: string, build: FwBuildView | null, flash: FwFlashView | null, };

export type FwEditInput = { "type": "set_fn", key: number, code: number | null, } | { "type": "set_default", key: number, code: number, } | { "type": "set_tap_hold", key: number, tap_hold: FwTapHoldView | null, } | { "type": "key_to_base", key: number, } | { "type": "key_to_stock", key: number, };

export type Snapshot = { phase: Phase, lang: UiLang, devices: Array<DeviceView>, device: DeviceView | null, model_id: string | null, 
/**
 * `layers[layer][i]` is the slot of `model.keys[i]`.
 */
layers: Array<Array<SlotView>>, changes: Array<ChangeView>, can_undo: boolean, can_redo: boolean, assist: AssistView, journal: JournalView | null, plan: PlanView | null, report: ReportView | null, 
/**
 * What the connected firmware reports it changed (command 0x30); `None` when it has no info block.
 */
firmware: FirmwareView | null, 
/**
 * Firmware mode (draft, build, flash); `None` until the keyboard was read.
 */
fw: FwView | null, error: ErrorView | null, simulated: boolean, };

export type BackupView = { name: string, path: string, created: string, reason: string | null, problem: string | null, };

export type ProgressView = { layer: number, 
/**
 * `read`, `write`, `verify` or `info`.
 */
phase: string, done: number, total: number, 
/**
 * Step of the whole operation, e.g. 3 of 8 page sessions.
 */
step: number, steps: number, };

export type KeyUsageView = { code: number, legend: string, 
/**
 * Shift character drawn above `legend` on the picker button.
 */
shifted: string | null, name: string, group: string, };

export type ConsumerUsageView = { code: number, legend: string, name: string, filco_assist: boolean, };

export type SystemUsageView = { code: number, legend: string, name: string, };

export type Catalog = { keys: Array<KeyUsageView>, consumer: Array<ConsumerUsageView>, system: Array<SystemUsageView>, 
/**
 * Firmware-only codes for Fn + key (mouse, volume), in Vietnamese and English.
 */
fw_special: Array<FwCodeView>, fw_special_en: Array<FwCodeView>, };

export type Action = { "type": "empty" } | { "type": "key", mods: number, usage: number, } | { "type": "mouse_click", buttons: number, } | { "type": "mouse_scroll", amount: number, } | { "type": "mouse_move", dx: number, dy: number, } | { "type": "consumer", usage: number, } | { "type": "system", usage: number, } | { "type": "raw", hex: string, };
