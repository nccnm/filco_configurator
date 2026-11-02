//! Decoder for the firmware info block.
//!
//! A patched V72 image can answer HID command 0x30 with a 512-byte block that says what the build changed:
//! its name, and a list of tap-hold keys, Fn-layer edits and default-keymap edits. The configurator reads this
//! block and displays it; it never builds or modifies firmware. This crate only parses the bytes.
//!
//! The format is the one designed and verified in the analysis repository (`rea_filco`, `fwinfo.py` and
//! `FN_ANALYSIS.md` section 14):
//!
//! ```text
//!  0  magic b"KLEI"
//!  4  format version (1)
//!  5  header length (16)
//!  6  used length u16 (header + entries + end marker)
//!  8  base firmware version u16 (0x0072)
//! 10  stock checksum tag u16 (0xF448)
//! 12  reserved (4 bytes)
//! 16  build name, 32 bytes, ASCII, zero padded
//! 48  entries: type u8, payload length u8, payload; type 0 ends the list
//! ```
//!
//! Entry payloads: `1` tap-hold key hold (key, tap, hold, term_ms u16); `2` tap-hold layer hold
//! (key, tap, layer 0-3, term_ms u16); `3` Fn layer (key, code, 0 = Fn function removed); `4` default keymap
//! (key, code); `5` layer tap-hold (slot type, 8): tap-hold keys are read from layer slots of that type, each
//! layer its own. An unknown type is skipped, so a newer build can add types without breaking this reader.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Where the block lives in the 64 KB image, for callers that read it from an image rather than over HID.
pub const BLOCK_ADDR: usize = 0xE600;
/// Block length: 8 chunks of 64 bytes, one ordinary read session.
pub const BLOCK_LEN: usize = 512;
/// HID command that returns the block.
pub const CMD_INFO: u8 = 0x30;

const MAGIC: &[u8; 4] = b"KLEI";
const FORMAT: u8 = 1;
const NAME_LEN: usize = 32;

const T_TAP_KEY: u8 = 1;
const T_TAP_LAYER: u8 = 2;
const T_FN: u8 = 3;
const T_KEYMAP: u8 = 4;
const T_LAYER_TAP: u8 = 5;

/// A tap-hold key whose hold sends another key (for example CapsLock: tap Esc, hold LCtrl).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapKey {
    pub key: u8,
    pub tap: u8,
    pub hold: u8,
    pub term_ms: u16,
}

/// A tap-hold key whose hold turns on a layer while held (for example Space: tap Space, hold layer 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapLayer {
    pub key: u8,
    pub tap: u8,
    /// Layer index 0-3 (shown to the user as 1-4).
    pub layer: u8,
    pub term_ms: u16,
}

/// A change to the Fn layer: `code` is `None` when the key's Fn function was removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnEdit {
    pub key: u8,
    pub code: Option<u8>,
}

/// A change to the default keymap of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeymapEdit {
    pub key: u8,
    pub code: u8,
}

/// A decoded info block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub format: u8,
    pub base_version: u16,
    pub stock_tag: u16,
    pub name: String,
    /// At most one key-hold tap-hold key.
    pub tap_key: Option<TapKey>,
    pub tap_layers: Vec<TapLayer>,
    pub fn_layer: Vec<FnEdit>,
    pub keymap: Vec<KeymapEdit>,
    /// Slot type the firmware reads as a tap-hold key in each layer (8), when it has layer tap-hold.
    #[serde(default)]
    pub layer_tap_hold: Option<u8>,
}

/// Why a block could not be decoded into a full [`Info`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// No block: too short, or the magic is absent. This is how a host tells stock firmware (no info) apart.
    NoBlock,
    /// A block with a format version this reader does not understand; the version is reported.
    UnknownFormat(u8),
}

fn u16_le(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

/// Parse a block as a host would, from the raw bytes of a 0x30 response (or `image[BLOCK_ADDR..][..BLOCK_LEN]`).
///
/// # Errors
/// [`ReadError::NoBlock`] when the bytes are not a block at all (how stock firmware is recognised), and
/// [`ReadError::UnknownFormat`] when the format version is newer than this reader.
pub fn parse(data: &[u8]) -> Result<Info, ReadError> {
    if data.len() < 16 || &data[..4] != MAGIC {
        return Err(ReadError::NoBlock);
    }
    let format = data[4];
    let header_len = data[5] as usize;
    let used = u16_le(data, 6).ok_or(ReadError::NoBlock)? as usize;
    let base_version = u16_le(data, 8).ok_or(ReadError::NoBlock)?;
    let stock_tag = u16_le(data, 10).ok_or(ReadError::NoBlock)?;
    if format != FORMAT {
        return Err(ReadError::UnknownFormat(format));
    }
    let name_bytes = data
        .get(header_len..header_len + NAME_LEN)
        .ok_or(ReadError::NoBlock)?;
    let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
    let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();

    let mut info = Info {
        format,
        base_version,
        stock_tag,
        name,
        tap_key: None,
        tap_layers: Vec::new(),
        fn_layer: Vec::new(),
        keymap: Vec::new(),
        layer_tap_hold: None,
    };

    // Entries start after the fixed name field, and run until the end marker or `used`.
    let mut pos = header_len + NAME_LEN;
    let limit = used.min(data.len());
    while pos + 2 <= limit {
        let t = data[pos];
        let n = data[pos + 1] as usize;
        if t == 0 {
            break;
        }
        let Some(p) = data.get(pos + 2..pos + 2 + n) else {
            break;
        };
        match t {
            T_TAP_KEY if n >= 5 => {
                info.tap_key = Some(TapKey {
                    key: p[0],
                    tap: p[1],
                    hold: p[2],
                    term_ms: u16::from_le_bytes([p[3], p[4]]),
                });
            }
            T_TAP_LAYER if n >= 5 => {
                info.tap_layers.push(TapLayer {
                    key: p[0],
                    tap: p[1],
                    layer: p[2],
                    term_ms: u16::from_le_bytes([p[3], p[4]]),
                });
            }
            T_FN if n >= 2 => {
                info.fn_layer.push(FnEdit {
                    key: p[0],
                    code: (p[1] != 0).then_some(p[1]),
                });
            }
            T_KEYMAP if n >= 2 => {
                info.keymap.push(KeymapEdit {
                    key: p[0],
                    code: p[1],
                });
            }
            T_LAYER_TAP if n >= 1 => info.layer_tap_hold = Some(p[0]),
            _ => {} // unknown type or short payload: skip, as a forward-compatible reader must
        }
        pos += 2 + n;
    }
    Ok(info)
}

/// Read the block out of a full 64 KB firmware image. Convenience over `parse(&image[BLOCK_ADDR..])`.
///
/// # Errors
/// Same as [`parse`], plus [`ReadError::NoBlock`] when the image is too small to contain the block.
pub fn from_image(image: &[u8]) -> Result<Info, ReadError> {
    let block = image
        .get(BLOCK_ADDR..BLOCK_ADDR + BLOCK_LEN)
        .ok_or(ReadError::NoBlock)?;
    parse(block)
}
