//! Firmware info block and the HID command 0x30 that reads it (port of `fwinfo.py`; analysis in
//! `FN_ANALYSIS.md` section 14 of the analysis repository). The block format is decoded by `filco-fwinfo`;
//! this module encodes it and patches the command into the image.
//!
//! - `info_known` replaces the call at 0xa50a to 0xa3aa ("is the command known?") and answers yes for 0x30.
//! - `info_dispatch` replaces `cmp r5, #0x13 ; beq 0xa62c` at 0xa5a0 and, for 0x30, does what the layer read
//!   0x10 does with the block address, then joins the common end 0xa7fa.
//!
//! Neither hook adds stack depth: `info_known` pushes nothing, `info_dispatch` leaves with a jump.

use filco_fwinfo::{BLOCK_ADDR, BLOCK_LEN, CMD_INFO};

use crate::Error;
use crate::v72::{Firmware, STOCK_TAG};

pub const CODE_ADDR: u32 = 0xE500;
const REGION_END: usize = 0xE800;
const MAGIC: &[u8; 4] = b"KLEI";
const FORMAT: u8 = 1;
const HEADER_LEN: usize = 16;
const NAME_LEN: usize = 32;
const BASE_VERSION: u16 = 0x0072;
const T_TAP_KEY: u8 = 1;
const T_TAP_LAYER: u8 = 2;
const T_FN: u8 = 3;
const T_KEYMAP: u8 = 4;

const KNOWN_CALL: u32 = 0xA50A;
const KNOWN_FN: u32 = 0xA3AA;
const DISPATCH_AT: u32 = 0xA5A0;
const DISPATCH_STOCK: [u8; 4] = [0x13, 0x2D, 0x43, 0xD0];
const CASE_13: u32 = 0xA62C;
const CHECK_LEN: u32 = 0xA380;
const SET_READ: u32 = 0xA370;
const READ_END: u32 = 0xA7FA;
const PAGE_CHUNKS: u32 = 8;

/// The assembly source.
pub fn source() -> String {
    let lit_known = KNOWN_FN | 1;
    let lit_case13 = CASE_13 | 1;
    let lit_end = READ_END | 1;
    format!(
        "
info_known:
    cmp r0, #{CMD_INFO}
    bne ik_stock
    movs r0, #1
    bx lr
ik_stock:
    ldr r1, lit_known
    bx r1

info_dispatch:
    cmp r5, #{CMD_INFO}
    beq id_info
    cmp r5, #0x13
    bne id_back
    ldr r0, lit_case13
    bx r0
id_back:
    bx lr
id_info:
    movs r0, #{PAGE_CHUNKS}
    bl {CHECK_LEN:#x}
    cmp r0, #0
    bne id_end
    adds r0, r4, #1
    uxth r1, r0
    movs r2, #1
    ldr r0, lit_block
    bl {SET_READ:#x}
id_end:
    ldr r0, lit_end
    bx r0

    .align 2
lit_known:  .word {lit_known:#x}
lit_case13: .word {lit_case13:#x}
lit_block:  .word {BLOCK_ADDR:#x}
lit_end:    .word {lit_end:#x}
"
    )
}

/// What the block lists. Keys are key indices; layers are 0-3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockContent {
    pub name: String,
    /// (key, tap, hold, term_ms)
    pub tap_key: Option<(u8, u8, u8, u16)>,
    /// (key, tap, layer, term_ms)
    pub tap_layers: Vec<(u8, u8, u8, u16)>,
    /// Fn + key code; `None` = Fn function removed.
    pub fn_layer: Vec<(u8, Option<u8>)>,
    pub keymap: Vec<(u8, u8)>,
}

/// Encodes the block (entries sorted by key within each type, as `fwinfo.block`).
pub fn block(c: &BlockContent) -> Result<Vec<u8>, Error> {
    let mut entries = Vec::new();
    if let Some((k, tap, hold, term)) = c.tap_key {
        entries.extend_from_slice(&[T_TAP_KEY, 5, k, tap, hold]);
        entries.extend_from_slice(&term.to_le_bytes());
    }
    for &(k, tap, layer, term) in &c.tap_layers {
        entries.extend_from_slice(&[T_TAP_LAYER, 5, k, tap, layer]);
        entries.extend_from_slice(&term.to_le_bytes());
    }
    let mut fl = c.fn_layer.clone();
    fl.sort_by_key(|e| e.0);
    for (k, code) in fl {
        entries.extend_from_slice(&[T_FN, 2, k, code.unwrap_or(0)]);
    }
    let mut km = c.keymap.clone();
    km.sort_by_key(|e| e.0);
    for (k, code) in km {
        entries.extend_from_slice(&[T_KEYMAP, 2, k, code]);
    }
    if !c.name.is_ascii() || c.name.len() > NAME_LEN {
        return Err(Error::Config("build name longer than 32 bytes".into()));
    }
    let used = HEADER_LEN + NAME_LEN + entries.len() + 2;
    if used > BLOCK_LEN {
        return Err(Error::Config(format!(
            "info block needs {used} bytes, has {BLOCK_LEN}"
        )));
    }
    let mut out = Vec::with_capacity(BLOCK_LEN);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&[FORMAT, u8::try_from(HEADER_LEN).unwrap_or(16)]);
    out.extend_from_slice(&u16::try_from(used).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&BASE_VERSION.to_le_bytes());
    out.extend_from_slice(&STOCK_TAG.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    let mut name = c.name.as_bytes().to_vec();
    name.resize(NAME_LEN, 0);
    out.extend_from_slice(&name);
    out.extend_from_slice(&entries);
    out.resize(BLOCK_LEN, 0);
    Ok(out)
}

/// Patches `fw` in place: code, block and the two hooks.
pub fn apply(fw: &mut Firmware, info: &[u8]) -> Result<(), Error> {
    let at = |a: u32| a as usize;
    let checks: [(u32, Vec<u8>, &str); 2] = [
        (
            KNOWN_CALL,
            thumb::bl(KNOWN_CALL, KNOWN_FN).to_vec(),
            "parser asks 0xa3aa whether the command is known",
        ),
        (
            DISPATCH_AT,
            DISPATCH_STOCK.to_vec(),
            "dispatch: cmp r5, #0x13 ; beq 0xa62c",
        ),
    ];
    for (addr, want, what) in &checks {
        if fw.b[at(*addr)..at(*addr) + want.len()] != want[..] {
            return Err(Error::Structure(format!(
                "stock structure not found at {addr:#x} ({what})"
            )));
        }
    }
    let p = thumb::assemble(&source(), CODE_ADDR).map_err(|e| Error::Asm(e.to_string()))?;
    let sym = |n: &str| p.symbol(n).map_err(|e| Error::Asm(e.to_string()));
    let (known, dispatch, lits) = (sym("info_known")?, sym("info_dispatch")?, sym("lit_known")?);
    crate::check_m0(&p.code[..at(lits - CODE_ADDR)], CODE_ADDR)?;
    let code = p.code.clone();
    if at(CODE_ADDR) + code.len() > BLOCK_ADDR || fw.b[at(CODE_ADDR)..REGION_END].iter().any(|&x| x != 0) {
        return Err(Error::Structure(format!(
            "region {CODE_ADDR:#x}-{:#x} is not free",
            REGION_END - 1
        )));
    }
    fw.put_bytes(
        at(CODE_ADDR),
        &code,
        &format!("info command {CMD_INFO:#04x} code ({} bytes)", code.len()),
    );
    fw.put_bytes(BLOCK_ADDR, info, &format!("info block ({BLOCK_LEN} bytes)"));
    let why = format!("parser: command {CMD_INFO:#04x} is known ({known:#x})");
    fw.put_bytes(at(KNOWN_CALL), &thumb::bl(KNOWN_CALL, known), &why);
    let why = format!("dispatch: command {CMD_INFO:#04x} reads the info block ({dispatch:#x})");
    fw.put_bytes(at(DISPATCH_AT), &thumb::bl(DISPATCH_AT, dispatch), &why);
    Ok(())
}
