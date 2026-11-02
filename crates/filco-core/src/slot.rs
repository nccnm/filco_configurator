//! The 4-byte key slot `[b0, b1, b2, b3]` of a layer page.
//!
//! The firmware dispatches on `b0 & 0x0f` (jump table at 0x76a0 in V60): 0 and 8-15 use the default keymap,
//! 1 mouse, 2 keystroke, 3 consumer (media), 4 system control, 5-7 other handlers (6 = macro). Type 8 is a
//! tap-hold key for firmware built with layer tap-hold (`filco-firmware` `layertap.rs`); stock firmware reads it
//! as the default keymap.
//!
//! [`Slot`] always keeps the exact four bytes. [`SlotKind`] is a view of them: [`Slot::kind`] maps every
//! possible byte pattern to exactly one kind, and [`SlotKind::to_slot`] maps it back to the same bytes, so
//! reading, displaying and writing a slot never changes bytes the editor does not understand.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hex;

/// Slot type codes (`b0 & 0x0f`) as the firmware uses them.
pub mod type_code {
    pub const EMPTY: u8 = 0;
    pub const MOUSE: u8 = 1;
    pub const KEY: u8 = 2;
    pub const CONSUMER: u8 = 3;
    pub const SYSTEM: u8 = 4;
    pub const MACRO: u8 = 6;
    pub const TAP_HOLD: u8 = 8;
}

/// `hold` values of a tap-hold slot from this one to `+ 3` turn on layer `hold - HOLD_LAYER` while held.
pub const HOLD_LAYER: u8 = 0xF0;
/// Term of a tap-hold slot whose term byte is 0, in 10 ms steps (200 ms).
pub const DEFAULT_TERM: u8 = 20;

/// Mouse sub-functions in `b1` of a type-1 slot (firmware 0x90aa).
pub mod mouse_code {
    pub const CLICK: u8 = 1;
    pub const SCROLL: u8 = 3;
    pub const MOVE: u8 = 4;
}

/// The four bytes of one key slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Slot(pub [u8; 4]);

impl Slot {
    pub const EMPTY: Slot = Slot([0; 4]);

    pub const fn bytes(self) -> [u8; 4] {
        self.0
    }

    /// The firmware type code, `b0 & 0x0f`.
    pub const fn type_code(self) -> u8 {
        self.0[0] & 0x0f
    }

    pub fn is_empty(self) -> bool {
        self == Self::EMPTY
    }

    /// Interprets the bytes. Total: every byte pattern has exactly one kind.
    pub fn kind(self) -> SlotKind {
        let [b0, b1, b2, b3] = self.0;
        match (b0, b1, b2, b3) {
            (0, 0, 0, 0) => SlotKind::Empty,
            (type_code::KEY, mods, usage, 0) => SlotKind::Key {
                mods: ModMask(mods),
                usage,
            },
            (type_code::MOUSE, mouse_code::CLICK, buttons, 0) => SlotKind::Mouse {
                action: MouseAction::Click { buttons },
            },
            (type_code::MOUSE, mouse_code::SCROLL, amount, 0) => SlotKind::Mouse {
                action: MouseAction::Scroll {
                    amount: i8::from_ne_bytes([amount]),
                },
            },
            (type_code::MOUSE, mouse_code::MOVE, dx, dy) => SlotKind::Mouse {
                action: MouseAction::Move {
                    dx: i8::from_ne_bytes([dx]),
                    dy: i8::from_ne_bytes([dy]),
                },
            },
            (type_code::CONSUMER, lo, hi, 0) => SlotKind::Consumer {
                usage: u16::from_le_bytes([lo, hi]),
            },
            (type_code::SYSTEM, usage, 0, 0) => SlotKind::System { usage },
            (type_code::MACRO, index, a, b) => SlotKind::Macro { index, extra: [a, b] },
            (type_code::TAP_HOLD, tap, hold, term) if hold <= HOLD_LAYER + 3 => {
                SlotKind::TapHold { tap, hold, term }
            }
            _ => SlotKind::Other { bytes: self.0 },
        }
    }
}

impl fmt::Debug for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Slot({})", hex::encode_spaced(&self.0))
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode_spaced(&self.0))
    }
}

impl std::str::FromStr for Slot {
    type Err = SlotParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|e| SlotParseError(e.to_string()))?;
        let arr: [u8; 4] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| SlotParseError(format!("{} bytes, expected 4", v.len())))?;
        Ok(Slot(arr))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid slot text: {0}")]
pub struct SlotParseError(String);

/// Serialized as `"02 00 a4 00"`, the form used in the analysis repository's documents.
impl Serialize for Slot {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode_spaced(&self.0))
    }
}

impl<'de> Deserialize<'de> for Slot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Modifier mask of a keystroke slot: bit i is HID usage `0xe0 + i`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModMask(pub u8);

impl ModMask {
    pub const LCTRL: u8 = 0x01;
    pub const LSHIFT: u8 = 0x02;
    pub const LALT: u8 = 0x04;
    pub const LWIN: u8 = 0x08;
    pub const RCTRL: u8 = 0x10;
    pub const RSHIFT: u8 = 0x20;
    pub const RALT: u8 = 0x40;
    pub const RWIN: u8 = 0x80;

    pub const fn contains(self, bit: u8) -> bool {
        self.0 & bit == bit
    }

    /// HID usages of the set modifiers, in bit order.
    pub fn usages(self) -> impl Iterator<Item = u8> {
        (0..8u8).filter(move |i| self.0 & (1 << i) != 0).map(|i| 0xe0 + i)
    }
}

impl fmt::Debug for ModMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ModMask({:#04x})", self.0)
    }
}

/// What a type-1 (mouse) slot does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum MouseAction {
    /// `01 01 buttons 00`; buttons is a bit mask (1 left, 2 right, 4 middle, 8 back, 0x10 forward).
    Click { buttons: u8 },
    /// `01 03 amount 00`; positive scrolls up.
    Scroll { amount: i8 },
    /// `01 04 dx dy`.
    Move { dx: i8, dy: i8 },
}

/// The meaning of a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotKind {
    /// `00 00 00 00`: the key does what the firmware's default keymap says.
    Empty,
    /// `02 mods usage 00`: press the modifiers then the HID keyboard usage (0 = modifiers only).
    Key {
        mods: ModMask,
        usage: u8,
    },
    Mouse {
        action: MouseAction,
    },
    /// `03 lo hi 00`: HID consumer usage, little-endian.
    Consumer {
        usage: u16,
    },
    /// `04 usage 00 00`: HID system control.
    System {
        usage: u8,
    },
    /// `06 index a b`: macro from the macro table (shown, not edited, in the first version).
    Macro {
        index: u8,
        extra: [u8; 2],
    },
    /// `08 tap hold term`: tap sends `tap`; held, sends `hold` (a key or modifier) or, for `F0`-`F3`, turns on
    /// layer 0-3; decided after `term` x 10 ms (0 = 200 ms). Needs firmware with layer tap-hold.
    TapHold {
        tap: u8,
        hold: u8,
        term: u8,
    },
    /// Any other pattern, kept byte for byte: types 5, 7, 9-15, a set high nibble in `b0`, a type-8 hold beyond
    /// layer 3, or a non-zero byte where the known layouts have zero.
    Other {
        bytes: [u8; 4],
    },
}

impl SlotKind {
    /// The bytes of this kind. Inverse of [`Slot::kind`].
    pub fn to_slot(self) -> Slot {
        Slot(match self {
            SlotKind::Empty => [0; 4],
            SlotKind::Key { mods, usage } => [type_code::KEY, mods.0, usage, 0],
            SlotKind::Mouse {
                action: MouseAction::Click { buttons },
            } => [type_code::MOUSE, mouse_code::CLICK, buttons, 0],
            SlotKind::Mouse {
                action: MouseAction::Scroll { amount },
            } => [type_code::MOUSE, mouse_code::SCROLL, amount.to_ne_bytes()[0], 0],
            SlotKind::Mouse {
                action: MouseAction::Move { dx, dy },
            } => [
                type_code::MOUSE,
                mouse_code::MOVE,
                dx.to_ne_bytes()[0],
                dy.to_ne_bytes()[0],
            ],
            SlotKind::Consumer { usage } => {
                let [lo, hi] = usage.to_le_bytes();
                [type_code::CONSUMER, lo, hi, 0]
            }
            SlotKind::System { usage } => [type_code::SYSTEM, usage, 0, 0],
            SlotKind::Macro { index, extra: [a, b] } => [type_code::MACRO, index, a, b],
            SlotKind::TapHold { tap, hold, term } => [type_code::TAP_HOLD, tap, hold, term],
            SlotKind::Other { bytes } => bytes,
        })
    }

    /// For a tap-hold slot: the layer (0-3) holding turns on, `None` when holding sends a key.
    pub fn hold_layer(self) -> Option<u8> {
        match self {
            SlotKind::TapHold { hold, .. } if hold >= HOLD_LAYER => Some(hold - HOLD_LAYER),
            _ => None,
        }
    }

    /// For a tap-hold slot: the decision time in ms.
    pub fn term_ms(self) -> Option<u16> {
        match self {
            SlotKind::TapHold { term, .. } => {
                Some(u16::from(if term == 0 { DEFAULT_TERM } else { term }) * 10)
            }
            _ => None,
        }
    }

    /// The firmware type code the bytes of this kind carry.
    pub fn type_code(self) -> u8 {
        self.to_slot().type_code()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn s(text: &str) -> Slot {
        text.parse().unwrap()
    }

    #[test]
    fn known_slots_from_the_real_keyboard() {
        // M10 = Fn on all layers (README section 4).
        assert_eq!(
            s("02 00 a4 00").kind(),
            SlotKind::Key {
                mods: ModMask(0),
                usage: 0xa4
            }
        );
        // Layer 4, P = Win + Shift + S (README section 15): mask 0x0a = LShift | LWin.
        assert_eq!(
            s("02 0a 16 00").kind(),
            SlotKind::Key {
                mods: ModMask(0x0a),
                usage: 0x16
            }
        );
        // Layer 3: click left / right, scroll up / down (README section 13).
        assert_eq!(
            s("01 01 01 00").kind(),
            SlotKind::Mouse {
                action: MouseAction::Click { buttons: 1 }
            }
        );
        assert_eq!(
            s("01 01 02 00").kind(),
            SlotKind::Mouse {
                action: MouseAction::Click { buttons: 2 }
            }
        );
        assert_eq!(
            s("01 03 01 00").kind(),
            SlotKind::Mouse {
                action: MouseAction::Scroll { amount: 1 }
            }
        );
        assert_eq!(
            s("01 03 ff 00").kind(),
            SlotKind::Mouse {
                action: MouseAction::Scroll { amount: -1 }
            }
        );
        // Original M10 slot before the change: Right Alt as a modifier-masked keystroke.
        assert_eq!(
            s("02 40 e6 00").kind(),
            SlotKind::Key {
                mods: ModMask(0x40),
                usage: 0xe6
            }
        );
        assert_eq!(Slot::EMPTY.kind(), SlotKind::Empty);
    }

    #[test]
    fn consumer_is_little_endian() {
        // Firmware 0x8f92 (V60) puts b1 in the low report byte and b2 in the high one.
        assert_eq!(s("03 e9 00 00").kind(), SlotKind::Consumer { usage: 0x00e9 });
        assert_eq!(s("03 23 02 00").kind(), SlotKind::Consumer { usage: 0x0223 });
        assert_eq!(SlotKind::Consumer { usage: 0x0223 }.to_slot(), s("03 23 02 00"));
    }

    #[test]
    fn unusual_patterns_are_kept_as_other() {
        for text in [
            "00 00 00 01",
            "12 00 04 00",
            "02 00 04 01",
            "05 00 00 00",
            "08 01 f4 03",
            "09 01 02 03",
            "04 81 01 00",
        ] {
            let slot = s(text);
            assert_eq!(slot.kind(), SlotKind::Other { bytes: slot.bytes() }, "{text}");
        }
    }

    #[test]
    fn tap_hold_slots() {
        let k = s("08 29 e0 14").kind();
        assert_eq!(
            k,
            SlotKind::TapHold {
                tap: 0x29,
                hold: 0xe0,
                term: 20
            }
        );
        assert_eq!((k.hold_layer(), k.term_ms()), (None, Some(200)));
        let l = s("08 2c f3 00").kind();
        assert_eq!((l.hold_layer(), l.term_ms()), (Some(3), Some(200)));
        assert_eq!(l.to_slot(), s("08 2c f3 00"));
    }

    #[test]
    fn mod_mask_usages() {
        assert_eq!(ModMask(0x0a).usages().collect::<Vec<_>>(), vec![0xe1, 0xe3]);
        assert!(ModMask(0x07).contains(ModMask::LCTRL | ModMask::LALT));
        assert!(!ModMask(0x07).contains(ModMask::RALT));
    }

    #[test]
    fn serde_uses_spaced_hex() {
        let json = serde_json::to_string(&s("02 0a 16 00")).unwrap();
        assert_eq!(json, "\"02 0a 16 00\"");
        assert_eq!(serde_json::from_str::<Slot>(&json).unwrap(), s("02 0a 16 00"));
        assert!(serde_json::from_str::<Slot>("\"02 0a 16\"").is_err());
    }

    /// Every one of the 2^32 byte patterns survives decode + encode. About 4 s in release mode; run by
    /// `scripts/check.ps1` with `--release --ignored exhaustive`.
    #[test]
    #[ignore = "exhaustive, run in release mode"]
    fn exhaustive_round_trip_all_byte_patterns() {
        for v in 0..=u32::MAX {
            let slot = Slot(v.to_le_bytes());
            assert_eq!(slot.kind().to_slot(), slot);
        }
    }

    #[test]
    fn round_trip_every_type_byte() {
        // All 256 values of b0 against a spread of the other bytes (exhaustive over b0 and b1).
        for b0 in 0..=255u8 {
            for b1 in 0..=255u8 {
                for (b2, b3) in [(0, 0), (0xa4, 0), (0xff, 0), (0, 1), (0x80, 0x7f), (0xff, 0xff)] {
                    let slot = Slot([b0, b1, b2, b3]);
                    assert_eq!(slot.kind().to_slot(), slot);
                }
            }
        }
    }

    fn any_kind() -> impl Strategy<Value = SlotKind> {
        prop_oneof![
            Just(SlotKind::Empty),
            (any::<u8>(), any::<u8>()).prop_map(|(m, u)| SlotKind::Key {
                mods: ModMask(m),
                usage: u
            }),
            any::<u8>().prop_map(|b| SlotKind::Mouse {
                action: MouseAction::Click { buttons: b }
            }),
            any::<i8>().prop_map(|a| SlotKind::Mouse {
                action: MouseAction::Scroll { amount: a }
            }),
            (any::<i8>(), any::<i8>()).prop_map(|(dx, dy)| SlotKind::Mouse {
                action: MouseAction::Move { dx, dy }
            }),
            any::<u16>().prop_map(|u| SlotKind::Consumer { usage: u }),
            any::<u8>().prop_map(|u| SlotKind::System { usage: u }),
            (any::<u8>(), any::<[u8; 2]>()).prop_map(|(i, e)| SlotKind::Macro { index: i, extra: e }),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

        #[test]
        fn bytes_round_trip(bytes in any::<[u8; 4]>()) {
            let slot = Slot(bytes);
            prop_assert_eq!(slot.kind().to_slot(), slot);
        }

        #[test]
        fn kinds_round_trip(kind in any_kind()) {
            prop_assert_eq!(kind.to_slot().kind(), kind);
        }

        #[test]
        fn text_round_trip(bytes in any::<[u8; 4]>()) {
            let slot = Slot(bytes);
            prop_assert_eq!(slot.to_string().parse::<Slot>().unwrap(), slot);
        }
    }
}
