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
    pub long: String,
    /// Differs from what the keyboard holds.
    pub changed: bool,
    /// The editor can produce this kind (types 0-4); others are shown and kept but not edited.
    pub editable: bool,
}

pub fn kind_name(k: SlotKind) -> &'static str {
    match k {
        SlotKind::Empty => "empty",
        SlotKind::Key { .. } => "key",
        SlotKind::Mouse { .. } => "mouse",
        SlotKind::Consumer { .. } => "consumer",
        SlotKind::System { .. } => "system",
        SlotKind::Macro { .. } => "macro",
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
            _ => (label.short, None),
        };
        SlotView {
            hex: slot.to_string(),
            kind: kind_name(kind).to_owned(),
            short,
            cap_sub,
            long: label.long,
            changed,
            editable: !matches!(kind, SlotKind::Macro { .. } | SlotKind::Other { .. }),
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

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
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
    /// `read`, `write` or `verify`.
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
        let json = serde_json::to_string(&Action::Key { mods: 1, usage: 4 }).unwrap();
        assert_eq!(json, r#"{"type":"key","mods":1,"usage":4}"#);
    }
}
