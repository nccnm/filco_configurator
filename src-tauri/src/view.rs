//! Everything the UI receives or sends. The TypeScript side is generated from these types
//! (`ui/src/bindings/ipc.ts`, see the `bindings_are_current` test), so the two can never disagree.

use filco_core::label::{Lang, modifier_names, slot_label};
use filco_core::model::{KeyTag, Support};
use filco_core::usage::{self, UsageGroup};
use filco_core::{ModMask, ModelDescriptor, MouseAction, Slot, SlotKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UiLang {
    Vi,
    En,
}

impl From<UiLang> for Lang {
    fn from(l: UiLang) -> Lang {
        match l {
            UiLang::Vi => Lang::Vi,
            UiLang::En => Lang::En,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// No FILCO configuration interface is connected.
    NoDevice,
    /// A FILCO keyboard without a model descriptor.
    Unsupported,
    /// Connected, pages not read yet (e.g. FILCO Assist was running).
    Connected,
    Ready,
    /// Reading the keyboard failed; `error` says why.
    ReadFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct KeyView {
    pub id: String,
    pub index: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub legend: String,
    /// Shift character printed above `legend` (`{` above `[`), when the key's default usage has one.
    pub shifted: Option<String>,
    pub m_key: bool,
    pub unverified: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LayerInfoView {
    pub label: String,
    pub led: String,
    pub led_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ModelView {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub support: String,
    pub layers: Vec<LayerInfoView>,
    pub keys: Vec<KeyView>,
    /// Size of the layout in key units.
    pub width: f32,
    pub height: f32,
    pub editable_types: Vec<u8>,
    pub notes: Vec<String>,
}

impl ModelView {
    pub fn new(m: &ModelDescriptor, lang: UiLang) -> Self {
        let keys: Vec<KeyView> = m
            .layout
            .keys
            .iter()
            .map(|k| KeyView {
                id: k.id.clone(),
                index: k.index,
                x: k.x,
                y: k.y,
                w: k.w,
                h: k.h,
                legend: k.legend.clone(),
                shifted: k
                    .default_usage
                    .and_then(|u| u8::try_from(u.0).ok())
                    .filter(|&u| usage::keyboard(u).is_some_and(|ku| ku.legend == k.legend))
                    .and_then(usage::shifted)
                    .map(str::to_owned),
                m_key: k.tags.contains(&KeyTag::MKey),
                unverified: k.tags.contains(&KeyTag::Unverified),
                note: k.note.clone(),
            })
            .collect();
        let width = keys.iter().map(|k| k.x + k.w).fold(0.0, f32::max);
        let height = keys.iter().map(|k| k.y + k.h).fold(0.0, f32::max);
        let li = usize::from(lang == UiLang::En);
        ModelView {
            id: m.id.clone(),
            name: m.name.clone(),
            vendor: m.vendor.clone(),
            support: match m.support {
                Support::Verified => "verified",
                Support::ReadOnly => "read_only",
                Support::Experimental => "experimental",
            }
            .to_owned(),
            layers: m
                .layers
                .iter()
                .map(|l| LayerInfoView {
                    label: l.label.clone(),
                    led: l.led.clone(),
                    led_name: l.led_name[li].clone(),
                })
                .collect(),
            keys,
            width,
            height,
            editable_types: m.slot_types.editable.clone(),
            notes: m.notes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SlotView {
    /// `"02 00 a4 00"`.
    pub hex: String,
    /// `empty`, `key`, `mouse`, `consumer`, `system`, `macro`, `other`.
    pub kind: String,
    /// Keycap text; empty for an empty slot (the UI shows the printed legend faintly).
    pub short: String,
    /// Small line above `short` on the keycap: the modifiers of a long key combination, whose key alone is then
    /// in `short` (so "Ctrl+Shift+Alt+E" stays readable).
    pub cap_sub: Option<String>,
    /// Shift character drawn above `short` for a plain digit or symbol key (`{` above `[`).
    pub cap_shift: Option<String>,
    pub long: String,
    /// Differs from what the keyboard holds.
    pub changed: bool,
    /// The editor can produce this kind (types 0-4, 8); others are shown and kept but not edited.
    pub editable: bool,
    /// For a tap-hold slot that turns on a layer while held: that layer, 0-3 (the key is bold there).
    pub hold_layer: Option<u8>,
}

pub fn kind_name(k: SlotKind) -> &'static str {
    match k {
        SlotKind::Empty => "empty",
        SlotKind::Key { .. } => "key",
        SlotKind::Mouse { .. } => "mouse",
        SlotKind::Consumer { .. } => "consumer",
        SlotKind::System { .. } => "system",
        SlotKind::Macro { .. } => "macro",
        SlotKind::TapHold { .. } => "tap_hold",
        SlotKind::Other { .. } => "other",
    }
}

/// Longest key combination drawn on one line of a keycap.
const CAP_CHARS: usize = 7;

/// Modifiers for the small line of a keycap: full names when they fit, else initials ("C+S+A" for Ctrl, Shift,
/// Alt); the inspector and the diff always show the full names.
fn cap_mods(names: &[&str]) -> String {
    let full = names.join("+");
    if full.chars().count() <= 9 {
        return full;
    }
    names
        .iter()
        .map(|n| match *n {
            "Ctrl" => "C",
            "Shift" => "S",
            "Alt" => "A",
            "Win" => "W",
            "RCtrl" => "RC",
            "RShift" => "RS",
            "RAlt" => "RA",
            "RWin" => "RW",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

impl SlotView {
    pub fn new(slot: Slot, changed: bool, lang: UiLang) -> Self {
        let kind = slot.kind();
        let label = slot_label(kind, lang.into());
        let (short, cap_sub) = match kind {
            SlotKind::Key { mods, usage }
                if mods.0 != 0 && usage != 0 && label.short.chars().count() > CAP_CHARS =>
            {
                (
                    usage::keyboard(usage).map_or_else(|| format!("0x{usage:02x}"), |u| u.legend.to_owned()),
                    Some(cap_mods(&modifier_names(mods))),
                )
            }
            // The tap on the main line, the hold small above it, as the keycap of a firmware tap-hold key.
            SlotKind::TapHold { tap, hold, .. } => (
                code_legend(tap),
                Some(
                    kind.hold_layer()
                        .map_or_else(|| code_legend(hold), |l| format!("\u{2192} L{}", l + 1)),
                ),
            ),
            _ => (label.short, None),
        };
        let cap_shift = match kind {
            SlotKind::Key { mods, usage } if mods.0 == 0 => usage::shifted(usage).map(str::to_owned),
            _ => None,
        };
        SlotView {
            hex: slot.to_string(),
            kind: kind_name(kind).to_owned(),
            short,
            cap_sub,
            cap_shift,
            long: label.long,
            changed,
            editable: !matches!(kind, SlotKind::Macro { .. } | SlotKind::Other { .. }),
            hold_layer: kind.hold_layer(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct DeviceView {
    pub path: String,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ChangeView {
    pub layer: usize,
    pub index: u8,
    /// Key id in the layout, `None` for a slot without a physical key.
    pub key: Option<String>,
    pub legend: Option<String>,
    pub old: SlotView,
    pub new: SlotView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LinkedLayerView {
    pub layer: usize,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AssistView {
    pub running: Vec<String>,
    /// The process list could not be read: writing is blocked.
    pub probe_error: Option<String>,
    /// Layers FILCO Assist would rebuild from a profile.
    pub linked: Vec<LinkedLayerView>,
    pub config_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct JournalLayerView {
    pub layer: usize,
    /// `original`, `target` or `unknown`.
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct JournalView {
    pub started: String,
    pub backup: String,
    pub layers: Vec<JournalLayerView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct PlanView {
    pub layers: Vec<usize>,
    pub changes: Vec<ChangeView>,
    pub backup: String,
    /// Layers of the plan that FILCO Assist links to a profile.
    pub linked: Vec<LinkedLayerView>,
    pub experimental: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct OutcomeView {
    pub layer: usize,
    /// `written`, `not_written`, `restored`, `restore_failed`, `unknown`.
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ReportView {
    pub succeeded: bool,
    pub consistent: bool,
    pub outcomes: Vec<OutcomeView>,
    pub error: Option<String>,
    pub backup: String,
    pub journal_kept: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ErrorView {
    /// Stable code the UI translates, e.g. `assist_running`, `device_changed`, `read_failed`.
    pub code: String,
    /// Technical detail (English), shown under the translated text.
    pub detail: String,
}

impl ErrorView {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(code: &str, detail: impl ToString) -> Self {
        ErrorView {
            code: code.to_owned(),
            detail: detail.to_string(),
        }
    }
}

/// One tap-hold key as read from the firmware info block, resolved for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct TapHoldView {
    pub index: u8,
    /// Key id on this model, if the index has a key.
    pub key: Option<String>,
    /// Keycap legend of the key.
    pub legend: Option<String>,
    /// What a quick tap sends (a key legend), the keycap's main label.
    pub tap: String,
    /// What holding does: a key legend, or `→ L3` when it turns on a layer. Drawn small above the tap.
    pub hold: String,
    /// True when holding turns on a layer (the key is bold on that layer).
    pub hold_is_layer: bool,
    /// Layer index 0-3 that holding turns on, when `hold_is_layer`.
    pub hold_layer: Option<u8>,
    pub term_ms: u16,
}

/// A Fn-layer or default-keymap change from the info block, for the inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FirmwareEditView {
    pub index: u8,
    pub key: Option<String>,
    pub legend: Option<String>,
    /// The code the change sets, as a legend; `None` means the Fn function was removed.
    pub code: Option<String>,
}

/// What the firmware info block says this build changed. `None` on the snapshot when the firmware has no block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FirmwareView {
    pub name: String,
    pub tap_holds: Vec<TapHoldView>,
    pub fn_edits: Vec<FirmwareEditView>,
    pub keymap_edits: Vec<FirmwareEditView>,
    /// The firmware reads tap-hold keys from the layer slots (type 8): the Tap-hold tab of the layer page works.
    pub layer_tap_hold: bool,
}

/// HID keyboard usage code to its keycap legend, or `0xNN` when unknown.
fn code_legend(code: u8) -> String {
    usage::keyboard(code).map_or_else(|| format!("0x{code:02x}"), |u| u.legend.to_owned())
}

impl FirmwareView {
    /// Builds the display view from a decoded info block, resolving matrix indices to keys of `model` and usage
    /// codes to legends. `layer_label(0..=3)` gives the short layer name (`L3`).
    pub fn from_info(info: &filco_fwinfo::Info, model: &ModelDescriptor) -> Self {
        let key_fields = |index: u8| {
            let k = model.key_by_index(index);
            (k.map(|k| k.id.clone()), k.map(|k| k.legend.clone()))
        };
        let mut tap_holds = Vec::new();
        if let Some(t) = &info.tap_key {
            let (key, legend) = key_fields(t.key);
            tap_holds.push(TapHoldView {
                index: t.key,
                key,
                legend,
                tap: code_legend(t.tap),
                hold: code_legend(t.hold),
                hold_is_layer: false,
                hold_layer: None,
                term_ms: t.term_ms,
            });
        }
        for t in &info.tap_layers {
            let (key, legend) = key_fields(t.key);
            tap_holds.push(TapHoldView {
                index: t.key,
                key,
                legend,
                tap: code_legend(t.tap),
                hold: format!("\u{2192} L{}", t.layer + 1),
                hold_is_layer: true,
                hold_layer: Some(t.layer),
                term_ms: t.term_ms,
            });
        }
        let edit = |index: u8, code: Option<u8>| {
            let (key, legend) = key_fields(index);
            FirmwareEditView {
                index,
                key,
                legend,
                code: code.map(code_legend),
            }
        };
        FirmwareView {
            name: info.name.clone(),
            tap_holds,
            fn_edits: info.fn_layer.iter().map(|f| edit(f.key, f.code)).collect(),
            keymap_edits: info.keymap.iter().map(|k| edit(k.key, Some(k.code))).collect(),
            layer_tap_hold: info.layer_tap_hold == Some(filco_core::slot::type_code::TAP_HOLD),
        }
    }
}

/// A firmware key code as drawn on a keycap: a legend, or an icon for the firmware's own mouse and volume codes
/// and Menu (as in the manual's Fn layer diagram).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwCodeView {
    pub code: u8,
    /// Keycap text; empty when `icon` is set.
    pub legend: String,
    /// `lclick`, `rclick`, `mup`, `mdown`, `mleft`, `mright`, `wheelup`, `wheeldown`, `mute`, `voldown`, `volup`,
    /// `menu`.
    pub icon: Option<String>,
    /// Full name for the inspector and tooltips.
    pub name: String,
}

/// Firmware-only codes of the V72 Fn layer (manual, Fn layer diagram): (code, icon, Vietnamese, English).
const FW_SPECIAL: [(u8, &str, &str, &str); 11] = [
    (0xF7, "lclick", "Chuột: nhấp trái", "Mouse: left click"),
    (0xF8, "rclick", "Chuột: nhấp phải", "Mouse: right click"),
    (0xFB, "mup", "Chuột: di lên", "Mouse: move up"),
    (0xFC, "mdown", "Chuột: di xuống", "Mouse: move down"),
    (0xF9, "mleft", "Chuột: di sang trái", "Mouse: move left"),
    (0xFA, "mright", "Chuột: di sang phải", "Mouse: move right"),
    (0xFD, "wheelup", "Chuột: cuộn lên", "Mouse: scroll up"),
    (0xFE, "wheeldown", "Chuột: cuộn xuống", "Mouse: scroll down"),
    (0xB5, "mute", "Tắt âm", "Mute"),
    (0xBA, "voldown", "Giảm âm lượng", "Volume down"),
    (0xB9, "volup", "Tăng âm lượng", "Volume up"),
];

impl FwCodeView {
    pub fn new(code: u8, lang: UiLang) -> Self {
        let en = lang == UiLang::En;
        if let Some(&(_, icon, vi, eng)) = FW_SPECIAL.iter().find(|s| s.0 == code) {
            return FwCodeView {
                code,
                legend: String::new(),
                icon: Some(icon.to_owned()),
                name: (if en { eng } else { vi }).to_owned(),
            };
        }
        let (legend, icon, name) = match code {
            0x65 => (String::new(), Some("menu".to_owned()), "Menu".to_owned()),
            filco_firmware::v72::FN_CODE => ("Fn".to_owned(), None, "Fn".to_owned()),
            0xD0..=0xD9 => {
                let m = format!("M{}", code - 0xCF);
                (m.clone(), None, m)
            }
            _ => {
                let label = slot_label(
                    SlotKind::Key {
                        mods: ModMask(0),
                        usage: code,
                    },
                    lang.into(),
                );
                (label.short, None, label.long)
            }
        };
        FwCodeView {
            code,
            legend,
            icon,
            name,
        }
    }

    /// The firmware-only codes, for the picker.
    pub fn specials(lang: UiLang) -> Vec<Self> {
        FW_SPECIAL.iter().map(|s| Self::new(s.0, lang)).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FwHoldView {
    Key {
        code: u8,
    },
    /// Layer 0-3.
    Layer {
        layer: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FwTapHoldView {
    pub tap: u8,
    pub hold: FwHoldView,
    pub term_ms: u16,
}

/// One key in firmware mode, in the order of `model.keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwKeyView {
    pub index: u8,
    /// Fn + key now; `None` when the key has no Fn function.
    pub fn_code: Option<FwCodeView>,
    pub fn_base: Option<FwCodeView>,
    pub fn_stock: Option<FwCodeView>,
    pub default: FwCodeView,
    pub default_base: FwCodeView,
    pub default_stock: FwCodeView,
    pub tap_hold: Option<FwTapHoldView>,
    /// Keycap texts of the tap-hold: hold (`Ctrl`, `→ L3`) above, tap in the middle.
    pub tap_label: Option<String>,
    pub hold_label: Option<String>,
    /// Differs from what the keyboard holds (not built and flashed yet).
    pub changed: bool,
    /// Differs from stock V72.
    pub custom: bool,
    /// `layout_specific`, `fn_key`, `tap_hold`.
    pub fn_locked: Option<String>,
    /// `has_fn`, `fn_key`.
    pub tap_hold_locked: Option<String>,
}

/// One step of a build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwStepView {
    /// `image`, `keys`, `fn`, `layers`, `tap_hold`, `info`, `stack`, `updater`.
    pub id: String,
    pub ok: u32,
    pub fail: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwBuildView {
    pub name: String,
    pub running: bool,
    pub steps: Vec<FwStepView>,
    /// Set when finished.
    pub passed: Option<bool>,
    pub tag: Option<u16>,
    pub dir: Option<String>,
    pub has_updater: bool,
    /// Failed report lines (when not passed).
    pub failures: Vec<String>,
    /// The whole report, for "show report".
    pub report: Vec<String>,
    /// The pipeline could not run (I/O, refused config).
    pub error: Option<String>,
    /// The draft changed after this build: it can no longer be flashed from here.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwFlashView {
    pub name: String,
    pub tag: u16,
    /// `ready` (backup done, Assist closed), `waiting` (updater started, waiting for the keyboard to come back),
    /// `done` (checked after flashing).
    pub step: String,
    pub backup: Option<String>,
    /// The keyboard went away after the updater started (it is being flashed).
    pub saw_unplug: bool,
    /// After flashing: the firmware on the keyboard is the build.
    pub firmware_ok: Option<bool>,
    /// After flashing: name the keyboard reports (`None`: no info block).
    pub firmware_name: Option<String>,
    /// After flashing: layers that differ from the backup.
    pub layers_changed: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwBuiltView {
    pub name: String,
    pub dir: String,
    pub tag: Option<u16>,
    pub passed: bool,
    /// Seconds since the epoch (a JSON number, well inside the safe integer range).
    #[ts(type = "number")]
    pub modified: u64,
    /// This build is what the keyboard runs now.
    pub on_keyboard: bool,
}

/// Firmware mode. `None` on the snapshot until the keyboard was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct FwView {
    /// The stock firmware shipped with the app could not be loaded: firmware mode is unavailable.
    pub stock_error: Option<String>,
    /// Name and checksum of the firmware on the keyboard; `None` when it has no info block (stock V72 or an
    /// older patched build).
    pub on_keyboard: Option<String>,
    pub on_keyboard_tag: Option<u16>,
    pub keys: Vec<FwKeyView>,
    /// Keys that differ from the keyboard.
    pub changes: u32,
    pub can_undo: bool,
    pub can_redo: bool,
    pub fn_slots_used: u32,
    pub fn_slots: u32,
    pub tap_key_used: u32,
    pub tap_layer_used: u32,
    /// The draft reads tap-hold keys from the layer slots (type 8).
    pub layer_tap_hold: bool,
    /// Index of the Fn key.
    pub fn_key: u8,
    pub suggested_name: String,
    pub build: Option<FwBuildView>,
    pub flash: Option<FwFlashView>,
}

/// A firmware edit from the inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FwEditInput {
    SetFn {
        key: u8,
        code: Option<u8>,
    },
    SetDefault {
        key: u8,
        code: u8,
    },
    SetTapHold {
        key: u8,
        tap_hold: Option<FwTapHoldView>,
    },
    KeyToBase {
        key: u8,
    },
    KeyToStock {
        key: u8,
    },
    SetLayerTapHold {
        on: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags the UI reads one by one"
)]
pub struct Snapshot {
    pub phase: Phase,
    pub lang: UiLang,
    pub devices: Vec<DeviceView>,
    pub device: Option<DeviceView>,
    pub model_id: Option<String>,
    /// `layers[layer][i]` is the slot of `model.keys[i]`.
    pub layers: Vec<Vec<SlotView>>,
    pub changes: Vec<ChangeView>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub assist: AssistView,
    pub journal: Option<JournalView>,
    pub plan: Option<PlanView>,
    pub report: Option<ReportView>,
    /// What the connected firmware reports it changed (command 0x30); `None` when it has no info block.
    pub firmware: Option<FirmwareView>,
    /// The layers hold tap-hold slots the firmware on the keyboard does not read yet: saving builds and flashes a
    /// firmware that does (once), after writing the layers.
    pub tap_hold_upgrade: bool,
    /// Firmware mode (draft, build, flash); `None` until the keyboard was read.
    pub fw: Option<FwView>,
    pub error: Option<ErrorView>,
    pub simulated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct BackupView {
    pub name: String,
    pub path: String,
    pub created: String,
    pub reason: Option<String>,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ProgressView {
    pub layer: u8,
    /// `read`, `write`, `verify` or `info`.
    pub phase: String,
    pub done: u8,
    pub total: u8,
    /// Step of the whole operation, e.g. 3 of 8 page sessions.
    pub step: u32,
    pub steps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct KeyUsageView {
    pub code: u8,
    pub legend: String,
    /// Shift character drawn above `legend` on the picker button.
    pub shifted: Option<String>,
    pub name: String,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ConsumerUsageView {
    pub code: u16,
    pub legend: String,
    pub name: String,
    pub filco_assist: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct SystemUsageView {
    pub code: u8,
    pub legend: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct Catalog {
    pub keys: Vec<KeyUsageView>,
    pub consumer: Vec<ConsumerUsageView>,
    pub system: Vec<SystemUsageView>,
    /// Firmware-only codes for Fn + key (mouse, volume), in Vietnamese and English.
    pub fw_special: Vec<FwCodeView>,
    pub fw_special_en: Vec<FwCodeView>,
}

fn group_name(g: UsageGroup) -> &'static str {
    match g {
        UsageGroup::Letter => "letter",
        UsageGroup::Digit => "digit",
        UsageGroup::Function => "function",
        UsageGroup::Navigation => "navigation",
        UsageGroup::Editing => "editing",
        UsageGroup::Symbol => "symbol",
        UsageGroup::Modifier => "modifier",
        UsageGroup::Keypad => "keypad",
        UsageGroup::International => "international",
        UsageGroup::Lock => "lock",
        UsageGroup::Special => "special",
    }
}

impl Catalog {
    pub fn new() -> Self {
        Catalog {
            keys: usage::KEYBOARD
                .iter()
                .map(|u| KeyUsageView {
                    code: u.code,
                    legend: u.legend.to_owned(),
                    shifted: usage::shifted(u.code).map(str::to_owned),
                    name: u.name.to_owned(),
                    group: group_name(u.group).to_owned(),
                })
                .collect(),
            consumer: usage::CONSUMER
                .iter()
                .map(|u| ConsumerUsageView {
                    code: u.code,
                    legend: u.legend.to_owned(),
                    name: u.name.to_owned(),
                    filco_assist: u.filco_assist,
                })
                .collect(),
            system: usage::SYSTEM
                .iter()
                .map(|u| SystemUsageView {
                    code: u.code,
                    legend: u.legend.to_owned(),
                    name: u.name.to_owned(),
                })
                .collect(),
            fw_special: FwCodeView::specials(UiLang::Vi),
            fw_special_en: FwCodeView::specials(UiLang::En),
        }
    }
}

/// What the user picked in the action panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Empty,
    Key {
        mods: u8,
        usage: u8,
    },
    MouseClick {
        buttons: u8,
    },
    MouseScroll {
        amount: i8,
    },
    MouseMove {
        dx: i8,
        dy: i8,
    },
    Consumer {
        usage: u16,
    },
    System {
        usage: u8,
    },
    /// Tap sends `tap`; held, sends `hold` (a key or modifier) or, for `0xF0 + n`, turns on layer n; `term` in
    /// 10 ms steps (0 = 200 ms).
    TapHold {
        tap: u8,
        hold: u8,
        term: u8,
    },
    /// Four bytes typed in the Advanced tab, e.g. `"02 00 a4 00"`.
    Raw {
        hex: String,
    },
}

impl Action {
    pub fn to_slot(&self) -> Result<Slot, String> {
        Ok(match *self {
            Action::Empty => SlotKind::Empty.to_slot(),
            Action::Key { mods, usage } => SlotKind::Key {
                mods: ModMask(mods),
                usage,
            }
            .to_slot(),
            Action::MouseClick { buttons } => SlotKind::Mouse {
                action: MouseAction::Click { buttons },
            }
            .to_slot(),
            Action::MouseScroll { amount } => SlotKind::Mouse {
                action: MouseAction::Scroll { amount },
            }
            .to_slot(),
            Action::MouseMove { dx, dy } => SlotKind::Mouse {
                action: MouseAction::Move { dx, dy },
            }
            .to_slot(),
            Action::Consumer { usage } => SlotKind::Consumer { usage }.to_slot(),
            Action::System { usage } => SlotKind::System { usage }.to_slot(),
            Action::TapHold { tap, hold, term } => SlotKind::TapHold { tap, hold, term }.to_slot(),
            Action::Raw { ref hex } => hex.parse::<Slot>().map_err(|e| e.to_string())?,
        })
    }
}

/// The generated TypeScript file.
#[cfg(test)]
pub fn typescript() -> String {
    let cfg = ts_rs::Config::new();
    let decls = [
        UiLang::decl(&cfg),
        Phase::decl(&cfg),
        KeyView::decl(&cfg),
        LayerInfoView::decl(&cfg),
        ModelView::decl(&cfg),
        SlotView::decl(&cfg),
        DeviceView::decl(&cfg),
        ChangeView::decl(&cfg),
        LinkedLayerView::decl(&cfg),
        AssistView::decl(&cfg),
        JournalLayerView::decl(&cfg),
        JournalView::decl(&cfg),
        PlanView::decl(&cfg),
        OutcomeView::decl(&cfg),
        ReportView::decl(&cfg),
        ErrorView::decl(&cfg),
        TapHoldView::decl(&cfg),
        FirmwareEditView::decl(&cfg),
        FirmwareView::decl(&cfg),
        FwCodeView::decl(&cfg),
        FwHoldView::decl(&cfg),
        FwTapHoldView::decl(&cfg),
        FwKeyView::decl(&cfg),
        FwStepView::decl(&cfg),
        FwBuildView::decl(&cfg),
        FwFlashView::decl(&cfg),
        FwBuiltView::decl(&cfg),
        FwView::decl(&cfg),
        FwEditInput::decl(&cfg),
        Snapshot::decl(&cfg),
        BackupView::decl(&cfg),
        ProgressView::decl(&cfg),
        KeyUsageView::decl(&cfg),
        ConsumerUsageView::decl(&cfg),
        SystemUsageView::decl(&cfg),
        Catalog::decl(&cfg),
        Action::decl(&cfg),
    ];
    let mut out = String::from(
        "// Generated from src-tauri/src/view.rs by the `bindings_are_current` test. Do not edit.\n\
         // Regenerate: $env:UPDATE_BINDINGS=1; cargo test -p keyboard-layer-editor bindings\n\n",
    );
    for d in decls {
        out.push_str("export ");
        out.push_str(&d);
        out.push_str("\n\n");
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_are_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/bindings/ipc.ts");
        let want = typescript();
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &want).unwrap();
        }
        let have = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            have == want,
            "ui/src/bindings/ipc.ts is stale; run with UPDATE_BINDINGS=1"
        );
    }

    #[test]
    fn actions_encode_like_the_codec() {
        let cases = [
            (Action::Empty, "00 00 00 00"),
            (
                Action::Key {
                    mods: 0x0a,
                    usage: 0x16,
                },
                "02 0a 16 00",
            ),
            (Action::MouseClick { buttons: 2 }, "01 01 02 00"),
            (Action::MouseScroll { amount: -1 }, "01 03 ff 00"),
            (Action::MouseMove { dx: 5, dy: -5 }, "01 04 05 fb"),
            (Action::Consumer { usage: 0xe9 }, "03 e9 00 00"),
            (Action::System { usage: 2 }, "04 02 00 00"),
            (
                Action::Raw {
                    hex: "02 00 a4 00".into(),
                },
                "02 00 a4 00",
            ),
        ];
        for (a, hex) in cases {
            assert_eq!(a.to_slot().unwrap().to_string(), hex, "{a:?}");
        }
        assert!(Action::Raw { hex: "zz".into() }.to_slot().is_err());
        let combo = SlotView::new("02 07 08 00".parse().unwrap(), false, UiLang::En);
        assert_eq!(
            (combo.short.as_str(), combo.cap_sub.as_deref()),
            ("E", Some("C+S+A"))
        );
        let two = SlotView::new("02 0a 16 00".parse().unwrap(), false, UiLang::En);
        assert_eq!(
            (two.short.as_str(), two.cap_sub.as_deref()),
            ("S", Some("Shift+Win"))
        );
        let short = SlotView::new("02 01 4f 00".parse().unwrap(), false, UiLang::En);
        assert_eq!(short.cap_sub, None);
        // A plain symbol key carries its Shift character; with a modifier the keycap shows the combination only.
        let bracket = SlotView::new("02 00 2f 00".parse().unwrap(), false, UiLang::En);
        assert_eq!(
            (bracket.short.as_str(), bracket.cap_shift.as_deref()),
            ("[", Some("{"))
        );
        let ctrl_bracket = SlotView::new("02 01 2f 00".parse().unwrap(), false, UiLang::En);
        assert_eq!(ctrl_bracket.cap_shift, None);
        let json = serde_json::to_string(&Action::Key { mods: 1, usage: 4 }).unwrap();
        assert_eq!(json, r#"{"type":"key","mods":1,"usage":4}"#);
    }

    #[test]
    fn printed_keys_carry_their_shift_character() {
        let m = filco_core::model::builtin::models().unwrap().remove(0);
        let v = ModelView::new(&m, UiLang::En);
        let shifted = |id: &str| v.keys.iter().find(|k| k.id == id).unwrap().shifted.clone();
        assert_eq!(shifted("[").as_deref(), Some("{"));
        assert_eq!(shifted("1").as_deref(), Some("!"));
        assert_eq!(shifted("Q"), None);
        assert_eq!(shifted("M1"), None);
    }
}
