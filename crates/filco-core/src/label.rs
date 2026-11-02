//! Human-readable text for slots, in Vietnamese or English.

use serde::{Deserialize, Serialize};

use crate::slot::{ModMask, MouseAction, SlotKind};
use crate::usage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    #[default]
    Vi,
    En,
}

/// Text for one slot: `short` fits on a keycap (may contain `\n`), `long` is for the inspector and the diff.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SlotLabel {
    pub short: String,
    pub long: String,
}

const MODS: [(u8, &str, &str); 8] = [
    (ModMask::LCTRL, "Ctrl", "Left Ctrl"),
    (ModMask::LSHIFT, "Shift", "Left Shift"),
    (ModMask::LALT, "Alt", "Left Alt"),
    (ModMask::LWIN, "Win", "Left Win"),
    (ModMask::RCTRL, "RCtrl", "Right Ctrl"),
    (ModMask::RSHIFT, "RShift", "Right Shift"),
    (ModMask::RALT, "RAlt", "Right Alt"),
    (ModMask::RWIN, "RWin", "Right Win"),
];

/// Modifier names in the conventional order Ctrl, Shift, Alt, Win, left before right.
pub fn modifier_names(mods: ModMask) -> Vec<&'static str> {
    MODS.iter()
        .filter(|(bit, _, _)| mods.contains(*bit))
        .map(|(_, short, _)| *short)
        .collect()
}

fn key_legend(code: u8) -> String {
    usage::keyboard(code).map_or_else(|| format!("0x{code:02x}"), |u| u.legend.to_owned())
}

fn key_name(code: u8) -> String {
    usage::keyboard(code).map_or_else(|| format!("HID 0x{code:02x}"), |u| u.name.to_owned())
}

fn mouse_buttons(buttons: u8, lang: Lang) -> (String, String) {
    const NAMES: [(u8, &str, &str, &str, &str); 5] = [
        (
            0x01,
            "click\ntrái",
            "Click chuột trái",
            "left\nclick",
            "Left click",
        ),
        (
            0x02,
            "click\nphải",
            "Click chuột phải",
            "right\nclick",
            "Right click",
        ),
        (
            0x04,
            "click\ngiữa",
            "Click chuột giữa",
            "middle\nclick",
            "Middle click",
        ),
        (
            0x08,
            "chuột\nlùi",
            "Nút lùi của chuột",
            "mouse\nback",
            "Mouse back button",
        ),
        (
            0x10,
            "chuột\ntiến",
            "Nút tiến của chuột",
            "mouse\nfwd",
            "Mouse forward button",
        ),
    ];
    let hits: Vec<_> = NAMES.iter().filter(|n| buttons & n.0 != 0).collect();
    let known: u8 = NAMES.iter().map(|n| n.0).fold(0, |a, b| a | b);
    if hits.len() == 1 && buttons & !known == 0 {
        let n = hits[0];
        return match lang {
            Lang::Vi => (n.1.to_owned(), n.2.to_owned()),
            Lang::En => (n.3.to_owned(), n.4.to_owned()),
        };
    }
    match lang {
        Lang::Vi => (
            format!("click\n0x{buttons:02x}"),
            format!("Click chuột, mặt nạ nút 0x{buttons:02x}"),
        ),
        Lang::En => (
            format!("click\n0x{buttons:02x}"),
            format!("Mouse click, button mask 0x{buttons:02x}"),
        ),
    }
}

fn arrow(dx: i8, dy: i8) -> Option<&'static str> {
    match (dx.signum(), dy.signum()) {
        (1, 0) => Some("→"),
        (-1, 0) => Some("←"),
        (0, 1) => Some("↓"),
        (0, -1) => Some("↑"),
        _ => None,
    }
}

fn key_text(mods: ModMask, usage: u8, vi: bool) -> (String, String) {
    let names = modifier_names(mods);
    match (usage, names.is_empty()) {
        (0, true) => (
            "∅".to_owned(),
            if vi {
                "Gõ phím: không có phím nào"
            } else {
                "Keystroke: no key"
            }
            .to_owned(),
        ),
        (0, false) => (names.join("+"), names.join(" + ")),
        (_, true) => (key_legend(usage), key_name(usage)),
        (_, false) => (
            format!("{}+{}", names.join("+"), key_legend(usage)),
            format!("{} + {}", names.join(" + "), key_name(usage)),
        ),
    }
}

fn scroll_text(amount: i8, vi: bool) -> (String, String) {
    let dir = if amount >= 0 { "↑" } else { "↓" };
    let n = amount.unsigned_abs();
    let word = if vi { "cuộn" } else { "scroll" };
    let short = if n == 1 {
        format!("{word} {dir}")
    } else {
        format!("{word} {dir}{n}")
    };
    let full = if vi {
        format!("Cuộn chuột {} {n} nấc", if amount >= 0 { "lên" } else { "xuống" })
    } else {
        format!("Scroll {} by {n}", if amount >= 0 { "up" } else { "down" })
    };
    (short, full)
}

fn move_text(dx: i8, dy: i8, vi: bool) -> (String, String) {
    let word = if vi { "chuột" } else { "mouse" };
    let short = arrow(dx, dy).map_or_else(|| format!("{word}\n{dx},{dy}"), |a| format!("{word} {a}"));
    let full = if vi {
        format!("Di chuột x {dx:+}, y {dy:+}")
    } else {
        format!("Move pointer x {dx:+}, y {dy:+}")
    };
    (short, full)
}

/// Label of a slot kind.
pub fn slot_label(kind: SlotKind, lang: Lang) -> SlotLabel {
    let vi = lang == Lang::Vi;
    let (short, full) = match kind {
        SlotKind::Empty => (
            String::new(),
            if vi {
                "Trống: theo keymap mặc định của firmware"
            } else {
                "Empty: firmware default keymap"
            }
            .to_owned(),
        ),
        SlotKind::Key { mods, usage } => key_text(mods, usage, vi),
        SlotKind::Mouse {
            action: MouseAction::Click { buttons },
        } => mouse_buttons(buttons, lang),
        SlotKind::Mouse {
            action: MouseAction::Scroll { amount },
        } => scroll_text(amount, vi),
        SlotKind::Mouse {
            action: MouseAction::Move { dx, dy },
        } => move_text(dx, dy, vi),
        SlotKind::Consumer { usage } => match usage::consumer(usage) {
            Some(u) => (u.legend.to_owned(), format!("Media: {}", u.name)),
            None => (
                format!("media\n0x{usage:04x}"),
                format!("Media (Consumer) 0x{usage:04x}"),
            ),
        },
        SlotKind::System { usage } => match usage::system(usage) {
            Some(u) => (u.legend.to_owned(), format!("System: {}", u.name)),
            None => (
                format!("sys\n0x{usage:02x}"),
                format!("System control 0x{usage:02x}"),
            ),
        },
        SlotKind::Macro { index, .. } => (
            format!("macro\n{index}"),
            if vi {
                format!("Macro số {index}")
            } else {
                format!("Macro {index}")
            },
        ),
        SlotKind::TapHold { tap, hold, .. } => {
            let ms = kind.term_ms().unwrap_or(0);
            let (hold_short, hold_long) = match kind.hold_layer() {
                Some(l) => (
                    format!("L{}", l + 1),
                    if vi {
                        format!("bật layer {}", l + 1)
                    } else {
                        format!("layer {}", l + 1)
                    },
                ),
                None => (key_legend(hold), key_name(hold)),
            };
            (
                format!(
                    "{}
{hold_short}",
                    key_legend(tap)
                ),
                if vi {
                    format!(
                        "Tap-hold: chạm {}, giữ {hold_long} (quyết định sau {ms} ms)",
                        key_name(tap)
                    )
                } else {
                    format!(
                        "Tap-hold: tap {}, hold {hold_long} (decided after {ms} ms)",
                        key_name(tap)
                    )
                },
            )
        }
        SlotKind::Other { bytes } => {
            let t = bytes[0] & 0x0f;
            let hex = crate::hex::encode_spaced(&bytes);
            (
                format!("{}\n{t}", if vi { "loại" } else { "type" }),
                if vi {
                    format!("Ô loại {t} chưa hỗ trợ: {hex}")
                } else {
                    format!("Unsupported type {t}: {hex}")
                },
            )
        }
    };
    SlotLabel { short, long: full }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Slot;

    fn label(text: &str, lang: Lang) -> SlotLabel {
        slot_label(text.parse::<Slot>().unwrap().kind(), lang)
    }

    #[test]
    fn keystrokes() {
        assert_eq!(label("02 00 a4 00", Lang::Vi).short, "Fn");
        assert_eq!(label("02 0a 16 00", Lang::En).short, "Shift+Win+S");
        assert_eq!(label("02 0a 16 00", Lang::En).long, "Shift + Win + S");
        assert_eq!(label("02 07 12 00", Lang::En).short, "Ctrl+Shift+Alt+O");
        assert_eq!(label("02 40 e6 00", Lang::En).long, "RAlt + Right Alt");
        assert_eq!(label("02 07 00 00", Lang::En).short, "Ctrl+Shift+Alt");
        assert_eq!(label("02 00 4f 00", Lang::Vi).short, "→");
        assert_eq!(label("02 00 f0 00", Lang::En).short, "0xf0");
    }

    #[test]
    fn mouse_and_media() {
        assert_eq!(label("01 01 01 00", Lang::Vi).short, "click\ntrái");
        assert_eq!(label("01 03 ff 00", Lang::Vi).short, "cuộn ↓");
        assert_eq!(label("01 03 01 00", Lang::En).long, "Scroll up by 1");
        assert_eq!(label("01 04 05 00", Lang::En).short, "mouse →");
        assert_eq!(label("01 04 05 05", Lang::En).short, "mouse\n5,5");
        assert_eq!(label("03 e9 00 00", Lang::En).short, "Vol +");
        assert_eq!(label("04 02 00 00", Lang::En).short, "Sleep");
        assert_eq!(label("06 02 00 00", Lang::Vi).long, "Macro số 2");
        assert_eq!(
            label("08 29 e0 14", Lang::En).short,
            "Esc
Ctrl"
        );
        assert_eq!(
            label("08 29 e0 14", Lang::En).long,
            "Tap-hold: tap Escape, hold Left Control (decided after 200 ms)"
        );
        assert_eq!(
            label("08 2c f2 1e", Lang::Vi).short,
            "Space
L3"
        );
        assert_eq!(
            label("08 33 e7 00", Lang::Vi).long,
            "Tap-hold: chạm ; :, giữ Right Windows (quyết định sau 200 ms)"
        );
        assert_eq!(
            label("07 01 02 03", Lang::En).long,
            "Unsupported type 7: 07 01 02 03"
        );
        assert_eq!(label("00 00 00 00", Lang::En).short, "");
    }

    #[test]
    fn modifier_order() {
        assert_eq!(
            modifier_names(ModMask(0xff)),
            ["Ctrl", "Shift", "Alt", "Win", "RCtrl", "RShift", "RAlt", "RWin"]
        );
    }
}
