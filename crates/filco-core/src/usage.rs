//! HID usage names (USB HID Usage Tables 1.5: Keyboard/Keypad page 0x07, Consumer page 0x0c, Generic Desktop
//! system controls 0x81-0x83), plus the FILCO-specific keyboard codes. Written for this project from the public
//! HID specification; nothing here is copied from Diatec software or firmware.

use serde::Serialize;

/// Group a usage belongs to in the action picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageGroup {
    Letter,
    Digit,
    Function,
    Navigation,
    Editing,
    Symbol,
    Modifier,
    Keypad,
    International,
    Lock,
    Special,
}

/// One keyboard-page usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct KeyUsage {
    pub code: u8,
    /// Short legend for a keycap, e.g. `"Bksp"`.
    pub legend: &'static str,
    /// Full English name, e.g. `"Backspace"`.
    pub name: &'static str,
    pub group: UsageGroup,
}

const fn k(code: u8, legend: &'static str, name: &'static str, group: UsageGroup) -> KeyUsage {
    KeyUsage {
        code,
        legend,
        name,
        group,
    }
}

use UsageGroup as G;

/// FILCO firmware code that makes a key act as Fn (verified on the M10SP: M10 slot `02 00 a4 00`).
pub const FN_USAGE: u8 = 0xa4;

/// Keyboard page usages offered by the editor, in picker order.
pub static KEYBOARD: &[KeyUsage] = &[
    k(0x04, "A", "A", G::Letter),
    k(0x05, "B", "B", G::Letter),
    k(0x06, "C", "C", G::Letter),
    k(0x07, "D", "D", G::Letter),
    k(0x08, "E", "E", G::Letter),
    k(0x09, "F", "F", G::Letter),
    k(0x0a, "G", "G", G::Letter),
    k(0x0b, "H", "H", G::Letter),
    k(0x0c, "I", "I", G::Letter),
    k(0x0d, "J", "J", G::Letter),
    k(0x0e, "K", "K", G::Letter),
    k(0x0f, "L", "L", G::Letter),
    k(0x10, "M", "M", G::Letter),
    k(0x11, "N", "N", G::Letter),
    k(0x12, "O", "O", G::Letter),
    k(0x13, "P", "P", G::Letter),
    k(0x14, "Q", "Q", G::Letter),
    k(0x15, "R", "R", G::Letter),
    k(0x16, "S", "S", G::Letter),
    k(0x17, "T", "T", G::Letter),
    k(0x18, "U", "U", G::Letter),
    k(0x19, "V", "V", G::Letter),
    k(0x1a, "W", "W", G::Letter),
    k(0x1b, "X", "X", G::Letter),
    k(0x1c, "Y", "Y", G::Letter),
    k(0x1d, "Z", "Z", G::Letter),
    k(0x1e, "1", "1 !", G::Digit),
    k(0x1f, "2", "2 @", G::Digit),
    k(0x20, "3", "3 #", G::Digit),
    k(0x21, "4", "4 $", G::Digit),
    k(0x22, "5", "5 %", G::Digit),
    k(0x23, "6", "6 ^", G::Digit),
    k(0x24, "7", "7 &", G::Digit),
    k(0x25, "8", "8 *", G::Digit),
    k(0x26, "9", "9 (", G::Digit),
    k(0x27, "0", "0 )", G::Digit),
    k(0x28, "Enter", "Enter", G::Editing),
    k(0x29, "Esc", "Escape", G::Editing),
    k(0x2a, "Bksp", "Backspace", G::Editing),
    k(0x2b, "Tab", "Tab", G::Editing),
    k(0x2c, "Space", "Space", G::Editing),
    k(0x2d, "-", "- _", G::Symbol),
    k(0x2e, "=", "= +", G::Symbol),
    k(0x2f, "[", "[ {", G::Symbol),
    k(0x30, "]", "] }", G::Symbol),
    k(0x31, "\\", "\\ |", G::Symbol),
    k(0x32, "#", "Non-US # ~", G::International),
    k(0x33, ";", "; :", G::Symbol),
    k(0x34, "'", "' \"", G::Symbol),
    k(0x35, "`", "` ~", G::Symbol),
    k(0x36, ",", ", <", G::Symbol),
    k(0x37, ".", ". >", G::Symbol),
    k(0x38, "/", "/ ?", G::Symbol),
    k(0x39, "Caps", "Caps Lock", G::Lock),
    k(0x3a, "F1", "F1", G::Function),
    k(0x3b, "F2", "F2", G::Function),
    k(0x3c, "F3", "F3", G::Function),
    k(0x3d, "F4", "F4", G::Function),
    k(0x3e, "F5", "F5", G::Function),
    k(0x3f, "F6", "F6", G::Function),
    k(0x40, "F7", "F7", G::Function),
    k(0x41, "F8", "F8", G::Function),
    k(0x42, "F9", "F9", G::Function),
    k(0x43, "F10", "F10", G::Function),
    k(0x44, "F11", "F11", G::Function),
    k(0x45, "F12", "F12", G::Function),
    k(0x46, "PrtSc", "Print Screen", G::Special),
    k(0x47, "ScrLk", "Scroll Lock", G::Lock),
    k(0x48, "Pause", "Pause", G::Special),
    k(0x49, "Ins", "Insert", G::Navigation),
    k(0x4a, "Home", "Home", G::Navigation),
    k(0x4b, "PgUp", "Page Up", G::Navigation),
    k(0x4c, "Del", "Delete", G::Editing),
    k(0x4d, "End", "End", G::Navigation),
    k(0x4e, "PgDn", "Page Down", G::Navigation),
    k(0x4f, "→", "Right Arrow", G::Navigation),
    k(0x50, "←", "Left Arrow", G::Navigation),
    k(0x51, "↓", "Down Arrow", G::Navigation),
    k(0x52, "↑", "Up Arrow", G::Navigation),
    k(0x53, "NumLk", "Num Lock", G::Lock),
    k(0x54, "KP /", "Keypad /", G::Keypad),
    k(0x55, "KP *", "Keypad *", G::Keypad),
    k(0x56, "KP -", "Keypad -", G::Keypad),
    k(0x57, "KP +", "Keypad +", G::Keypad),
    k(0x58, "KP ↵", "Keypad Enter", G::Keypad),
    k(0x59, "KP 1", "Keypad 1", G::Keypad),
    k(0x5a, "KP 2", "Keypad 2", G::Keypad),
    k(0x5b, "KP 3", "Keypad 3", G::Keypad),
    k(0x5c, "KP 4", "Keypad 4", G::Keypad),
    k(0x5d, "KP 5", "Keypad 5", G::Keypad),
    k(0x5e, "KP 6", "Keypad 6", G::Keypad),
    k(0x5f, "KP 7", "Keypad 7", G::Keypad),
    k(0x60, "KP 8", "Keypad 8", G::Keypad),
    k(0x61, "KP 9", "Keypad 9", G::Keypad),
    k(0x62, "KP 0", "Keypad 0", G::Keypad),
    k(0x63, "KP .", "Keypad .", G::Keypad),
    k(0x64, "\\|", "Non-US \\ |", G::International),
    k(0x65, "Menu", "Menu (Application)", G::Special),
    k(0x67, "KP =", "Keypad =", G::Keypad),
    k(0x68, "F13", "F13", G::Function),
    k(0x69, "F14", "F14", G::Function),
    k(0x6a, "F15", "F15", G::Function),
    k(0x6b, "F16", "F16", G::Function),
    k(0x6c, "F17", "F17", G::Function),
    k(0x6d, "F18", "F18", G::Function),
    k(0x6e, "F19", "F19", G::Function),
    k(0x6f, "F20", "F20", G::Function),
    k(0x70, "F21", "F21", G::Function),
    k(0x71, "F22", "F22", G::Function),
    k(0x72, "F23", "F23", G::Function),
    k(0x73, "F24", "F24", G::Function),
    k(0x87, "Ro", "International 1 (Ro)", G::International),
    k(
        0x88,
        "Kana",
        "International 2 (Katakana/Hiragana)",
        G::International,
    ),
    k(0x89, "¥", "International 3 (Yen)", G::International),
    k(0x8a, "Henk", "International 4 (Henkan)", G::International),
    k(0x8b, "Muhen", "International 5 (Muhenkan)", G::International),
    k(0xe0, "Ctrl", "Left Control", G::Modifier),
    k(0xe1, "Shift", "Left Shift", G::Modifier),
    k(0xe2, "Alt", "Left Alt", G::Modifier),
    k(0xe3, "Win", "Left Windows", G::Modifier),
    k(0xe4, "RCtrl", "Right Control", G::Modifier),
    k(0xe5, "RShift", "Right Shift", G::Modifier),
    k(0xe6, "RAlt", "Right Alt", G::Modifier),
    k(0xe7, "RWin", "Right Windows", G::Modifier),
    k(FN_USAGE, "Fn", "Fn (FILCO)", G::Special),
];

pub fn keyboard(code: u8) -> Option<&'static KeyUsage> {
    KEYBOARD.iter().find(|u| u.code == code)
}

/// The character a digit or symbol key types with Shift on the US layout (the layout of every supported model),
/// drawn above the plain character on keycaps, as printed on the keyboard: `[` with `{`.
pub fn shifted(code: u8) -> Option<&'static str> {
    Some(match code {
        0x1e => "!",
        0x1f => "@",
        0x20 => "#",
        0x21 => "$",
        0x22 => "%",
        0x23 => "^",
        0x24 => "&",
        0x25 => "*",
        0x26 => "(",
        0x27 => ")",
        0x2d => "_",
        0x2e => "+",
        0x2f => "{",
        0x30 => "}",
        0x31 => "|",
        0x33 => ":",
        0x34 => "\"",
        0x35 => "~",
        0x36 => "<",
        0x37 => ">",
        0x38 => "?",
        _ => return None,
    })
}

/// One consumer-page usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ConsumerUsage {
    pub code: u16,
    pub legend: &'static str,
    pub name: &'static str,
    /// Offered by FILCO Assist 2.0.97 (decoder 0x6ed810), so known to work with this firmware family.
    pub filco_assist: bool,
}

const fn c(code: u16, legend: &'static str, name: &'static str, filco_assist: bool) -> ConsumerUsage {
    ConsumerUsage {
        code,
        legend,
        name,
        filco_assist,
    }
}

pub static CONSUMER: &[ConsumerUsage] = &[
    c(0x00cd, "Play", "Play / Pause", true),
    c(0x00b5, "Next", "Next Track", true),
    c(0x00b6, "Prev", "Previous Track", true),
    c(0x00b7, "Stop", "Stop", true),
    c(0x00e2, "Mute", "Mute", true),
    c(0x00e9, "Vol +", "Volume Up", true),
    c(0x00ea, "Vol −", "Volume Down", true),
    c(0x006f, "Bri +", "Brightness Up", false),
    c(0x0070, "Bri −", "Brightness Down", false),
    c(0x0183, "Media", "Media Player", false),
    c(0x018a, "Mail", "Mail", false),
    c(0x0192, "Calc", "Calculator", false),
    c(0x0194, "Files", "File Explorer (My Computer)", false),
    c(0x0221, "Search", "Browser Search", false),
    c(0x0223, "Web", "Browser Home", false),
    c(0x0224, "Back", "Browser Back", false),
    c(0x0225, "Fwd", "Browser Forward", false),
    c(0x0227, "Reload", "Browser Refresh", false),
    c(0x022a, "Fav", "Browser Favorites", false),
];

pub fn consumer(code: u16) -> Option<&'static ConsumerUsage> {
    CONSUMER.iter().find(|u| u.code == code)
}

/// One system control action of a type-4 slot. The keyboard's System Control input report (interface MI_01,
/// collection 2) is one byte of Variable buttons for usages 0x81..=0x83, and the firmware copies slot byte b1 into it
/// unchanged, so `code` is a bit mask, not the HID usage id (read from the report descriptor on a real M10SP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SystemUsage {
    pub code: u8,
    pub legend: &'static str,
    pub name: &'static str,
}

pub static SYSTEM: &[SystemUsage] = &[
    SystemUsage {
        code: 0x01,
        legend: "Power",
        name: "System Power Down",
    },
    SystemUsage {
        code: 0x02,
        legend: "Sleep",
        name: "System Sleep",
    },
    SystemUsage {
        code: 0x04,
        legend: "Wake",
        name: "System Wake Up",
    },
];

pub fn system(code: u8) -> Option<&'static SystemUsage> {
    SYSTEM.iter().find(|u| u.code == code)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn tables_have_unique_codes_and_text() {
        let mut seen = HashSet::new();
        for u in KEYBOARD {
            assert!(seen.insert(u.code), "duplicate keyboard usage {:#x}", u.code);
            assert!(!u.legend.is_empty() && !u.name.is_empty());
        }
        let mut seen = HashSet::new();
        for u in CONSUMER {
            assert!(seen.insert(u.code), "duplicate consumer usage {:#x}", u.code);
        }
        assert_eq!(CONSUMER.iter().filter(|u| u.filco_assist).count(), 7);
    }

    #[test]
    fn shifted_characters_match_the_names() {
        // Every digit and US symbol key has its Shift character, the same one its full name lists ("[ {").
        for u in KEYBOARD
            .iter()
            .filter(|u| matches!(u.group, UsageGroup::Digit | UsageGroup::Symbol))
        {
            let s = shifted(u.code).unwrap_or_else(|| panic!("no shifted character for {}", u.name));
            assert_eq!(u.name, format!("{} {s}", u.legend), "0x{:02x}", u.code);
        }
        assert_eq!(shifted(0x04), None, "letters show one legend");
        assert_eq!(shifted(0x32), None, "non-US keys are not on the US layout");
    }

    #[test]
    fn lookups() {
        assert_eq!(keyboard(0x29).unwrap().name, "Escape");
        assert_eq!(keyboard(FN_USAGE).unwrap().legend, "Fn");
        assert!(keyboard(0x00).is_none());
        assert_eq!(consumer(0x00e9).unwrap().legend, "Vol +");
        assert_eq!(system(0x02).unwrap().legend, "Sleep");
    }
}
