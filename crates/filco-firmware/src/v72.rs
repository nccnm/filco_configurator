//! What the build knows about the stock Majestouch Xacro M10SP US firmware V72 image (port of `v72.py`).
//!
//! All addresses are flash offsets in the 64 KB image (`FN_ANALYSIS.md` sections 7 and 8 of the analysis
//! repository). Every patch operation checks the stock structure first and refuses anything it cannot explain.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::Error;

pub const STOCK_IMAGE_SHA256: &str = "5b09f9c1652286bd4dd648455d3f91e86114b7d2c3119c550e23ccedfdcb2cfa";
pub const STOCK_HEX_SHA256: &str = "4d0a2cfb537416235ebea7aec056108aa77aebbe0ad8cda6ea85291ef77bb837";
pub const STOCK_UPDATER_SHA256: &str = "b3a5f385d5da36f9a9bcc730b286ad6dc82b6e8340cd29e1fcf90fc1eeb641b2";
/// File-name tag = 16-bit word sum of the image + 0x5E.
pub const TAG_OFFSET: u16 = 0x5E;
/// Tag of the stock image.
pub const STOCK_TAG: u16 = 0xF448;

/// Fn tables (39 pairs of normal, Fn code) by layout.
pub const FN_TABLES: [(&str, usize); 3] = [("QWERTY", 0xC6A2), ("Dvorak", 0xC6F0), ("Colemak", 0xC73E)];
pub const FN_ENTRIES: u8 = 39;
/// Index keymaps copied to RAM at boot, with the layout of the Fn table each one is used with.
pub const KEYMAPS: [(usize, &str); 5] = [
    (0xC7EC, "QWERTY"),
    (0xC9EC, "Dvorak"),
    (0xCBEC, "Colemak"),
    (0xCDEC, "QWERTY"),
    (0xCFEC, "QWERTY"),
];
pub const NAME_KEYMAP: usize = 0xC7EC;
/// "Key has an Fn function" bit per key index, two variants.
pub const BITMAPS: [usize; 2] = [0xC634, 0xC644];
/// Boot code 0x0166: `movs r0, #73 ; strb r0, [0x200000a8]`. The macro recorder and Ctrl + Fn identify the Fn
/// key by this position, not by its code.
pub const FN_POS_IMM: usize = 0x0166;
pub const FN_CODE: u8 = 0xA4;

/// Key code names in the order of `v72.CODES`, without "none".
pub fn code_table() -> &'static [(String, u8)] {
    use std::sync::OnceLock;
    static T: OnceLock<Vec<(String, u8)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t: Vec<(String, u8)> = Vec::new();
        for (i, ch) in ('A'..='Z').enumerate() {
            t.push((ch.to_string(), 0x04 + u8::try_from(i).unwrap_or(0)));
        }
        for (i, ch) in "1234567890".chars().enumerate() {
            t.push((ch.to_string(), 0x1E + u8::try_from(i).unwrap_or(0)));
        }
        for i in 0..12u8 {
            t.push((format!("F{}", i + 1), 0x3A + i));
        }
        let named: [(&str, u8); 46] = [
            ("Enter", 0x28),
            ("Esc", 0x29),
            ("Backspace", 0x2A),
            ("Tab", 0x2B),
            ("Space", 0x2C),
            ("Minus", 0x2D),
            ("Equal", 0x2E),
            ("LBracket", 0x2F),
            ("RBracket", 0x30),
            ("Backslash", 0x31),
            ("NonUSHash", 0x32),
            ("Semicolon", 0x33),
            ("Quote", 0x34),
            ("Grave", 0x35),
            ("Comma", 0x36),
            ("Period", 0x37),
            ("Slash", 0x38),
            ("CapsLock", 0x39),
            ("PrintScreen", 0x46),
            ("ScrollLock", 0x47),
            ("Pause", 0x48),
            ("Insert", 0x49),
            ("Home", 0x4A),
            ("PageUp", 0x4B),
            ("Delete", 0x4C),
            ("End", 0x4D),
            ("PageDown", 0x4E),
            ("Right", 0x4F),
            ("Left", 0x50),
            ("Down", 0x51),
            ("Up", 0x52),
            ("NumLock", 0x53),
            ("NonUSBackslash", 0x64),
            ("Menu", 0x65),
            ("LCtrl", 0xE0),
            ("LShift", 0xE1),
            ("LAlt", 0xE2),
            ("LWin", 0xE3),
            ("RCtrl", 0xE4),
            ("RShift", 0xE5),
            ("RAlt", 0xE6),
            ("RWin", 0xE7),
            ("Fn", 0xA4),
            ("M1", 0xD0),
            ("M2", 0xD1),
            ("M3", 0xD2),
        ];
        t.extend(named.iter().map(|(n, c)| ((*n).to_string(), *c)));
        for i in 3..10u8 {
            t.push((format!("M{}", i + 1), 0xD0 + i));
        }
        t
    })
}

/// Name of a key code (`-` for none, `0xNN` when it has no name).
pub fn code_name(c: Option<u8>) -> String {
    match c {
        None => "-".to_string(),
        Some(c) => code_table()
            .iter()
            .find(|(_, v)| *v == c)
            .map_or_else(|| format!("{c:#04x}"), |(n, _)| n.clone()),
    }
}

/// Parses a key code: a name (case-insensitive) or `0xNN`; "none" gives `None` when allowed.
pub fn parse_code(text: &str, allow_none: bool) -> Result<Option<u8>, Error> {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    if let Some(h) = lower.strip_prefix("0x") {
        return match u32::from_str_radix(h, 16) {
            Ok(v) if v > 0 && v < 0x100 => Ok(Some(u8::try_from(v).unwrap_or(0))),
            _ => Err(Error::Config(format!("code out of range: {t}"))),
        };
    }
    if lower == "none" {
        return if allow_none {
            Ok(None)
        } else {
            Err(Error::Config("'none' is only allowed in [fn_layer]".into()))
        };
    }
    code_table()
        .iter()
        .find(|(n, _)| n.to_ascii_lowercase() == lower)
        .map(|(_, c)| Some(*c))
        .ok_or_else(|| {
            Error::Config(format!(
                "unknown key code name: '{t}' (use a name such as Left, LAlt, F5, or 0xNN)"
            ))
        })
}

/// One changed byte and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub addr: usize,
    pub old: u8,
    pub new: u8,
    pub why: String,
}

/// A V72 image being patched, and a log of every byte changed.
#[derive(Debug, Clone)]
pub struct Firmware {
    pub stock: Vec<u8>,
    pub b: Vec<u8>,
    pub log: Vec<Change>,
    /// Key index -> name ("H", "Space#77"), by the default bitmap variant.
    pub names: BTreeMap<u8, String>,
}

/// SHA-256 as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn layout_table(layout: &str) -> usize {
    FN_TABLES
        .iter()
        .find(|(l, _)| *l == layout)
        .map_or(0, |(_, t)| *t)
}

impl Firmware {
    /// Starts from the stock image; anything else is refused.
    pub fn new(img: &[u8]) -> Result<Self, Error> {
        if sha256_hex(img) != STOCK_IMAGE_SHA256 {
            return Err(Error::NotStock);
        }
        let mut fw = Self {
            stock: img.to_vec(),
            b: img.to_vec(),
            log: Vec::new(),
            names: BTreeMap::new(),
        };
        fw.check_structure()?;
        fw.names = fw.key_names();
        Ok(fw)
    }

    fn check_structure(&self) -> Result<(), Error> {
        if self.b[FN_POS_IMM..FN_POS_IMM + 2] != [73, 0x20] {
            return Err(Error::Structure("boot Fn position instruction not found".into()));
        }
        for (km, _) in KEYMAPS {
            for k in 0..128usize {
                let (lo, hi) = (self.b[km + 2 * k], self.b[km + 2 * k + 1]);
                if usize::from(hi) != k && (lo, hi) != (0, 0xFF) {
                    return Err(Error::Structure(format!(
                        "keymap {km:#x} entry {k} has unexpected key index byte"
                    )));
                }
            }
        }
        Ok(())
    }

    fn bit(&self, bm: usize, k: u8) -> bool {
        (self.b[bm + usize::from(k >> 3)] >> (k & 7)) & 1 == 1
    }

    /// Whether key `k` has a Fn function; `None` when the two bitmap variants disagree.
    pub fn capable(&self, k: u8) -> Option<bool> {
        let (a, b) = (self.bit(BITMAPS[0], k), self.bit(BITMAPS[1], k));
        (a == b).then_some(a)
    }

    pub fn fn_keys(&self) -> Vec<u8> {
        (0..128).filter(|&k| self.capable(k) == Some(true)).collect()
    }

    /// Fn entry (or plain code) of key `k` in the name keymap.
    pub fn entry(&self, k: u8) -> u8 {
        self.b[NAME_KEYMAP + 2 * usize::from(k)]
    }

    fn name(&self, k: u8) -> &str {
        self.names.get(&k).map_or("?", String::as_str)
    }

    fn checked_entry(&self, k: u8) -> Result<u8, Error> {
        let first = self.b[KEYMAPS[0].0 + 2 * usize::from(k)];
        if KEYMAPS
            .iter()
            .any(|(km, _)| self.b[km + 2 * usize::from(k)] != first)
            || self.entry(k) >= FN_ENTRIES
        {
            return Err(Error::Config(format!(
                "{}: Fn entry differs between keymaps (layout-specific key); not supported",
                self.name(k)
            )));
        }
        Ok(self.entry(k))
    }

    /// Plain code of key `k` in keymap `km` used with `layout`.
    pub fn normal(&self, k: u8, km: usize, layout: &str) -> u8 {
        if self.capable(k) == Some(true) {
            self.b[layout_table(layout) + 2 * usize::from(self.entry(k))]
        } else {
            self.b[km + 2 * usize::from(k)]
        }
    }

    /// Fn + key code in the QWERTY layout, when the key has a Fn function.
    pub fn fn_output(&self, k: u8) -> Option<u8> {
        (self.capable(k) == Some(true))
            .then(|| self.b[layout_table("QWERTY") + 2 * usize::from(self.entry(k)) + 1])
    }

    /// Key index the boot code records as the Fn key.
    pub fn fn_position(&self) -> u8 {
        self.b[FN_POS_IMM]
    }

    fn key_names(&self) -> BTreeMap<u8, String> {
        let mut by_code: Vec<(u8, Vec<u8>)> = Vec::new();
        for k in 0..128u8 {
            let c = if self.bit(BITMAPS[0], k) {
                self.b[layout_table("QWERTY") + 2 * usize::from(self.entry(k))]
            } else {
                self.b[NAME_KEYMAP + 2 * usize::from(k)]
            };
            if c != 0 {
                match by_code.iter_mut().find(|(cc, _)| *cc == c) {
                    Some((_, keys)) => keys.push(k),
                    None => by_code.push((c, vec![k])),
                }
            }
        }
        let mut names = BTreeMap::new();
        for (c, keys) in by_code {
            let base = code_name(Some(c));
            for &k in &keys {
                names.insert(
                    k,
                    if keys.len() == 1 {
                        base.clone()
                    } else {
                        format!("{base}#{k}")
                    },
                );
            }
        }
        names
    }

    /// Key index by name ("H", "Space#77") or "#index".
    pub fn key(&self, text: &str) -> Result<u8, Error> {
        let t = text.trim();
        if let Some(n) = t.strip_prefix('#') {
            let k = n.parse::<u8>().ok().filter(|k| self.names.contains_key(k));
            return k.ok_or_else(|| Error::Config(format!("no key at index {n}")));
        }
        let lower = t.to_ascii_lowercase();
        if let Some((k, _)) = self.names.iter().find(|(_, n)| n.to_ascii_lowercase() == lower) {
            return Ok(*k);
        }
        let mut dup: Vec<&String> = self
            .names
            .values()
            .filter(|n| n.to_ascii_lowercase().starts_with(&format!("{lower}#")))
            .collect();
        dup.sort();
        if !dup.is_empty() {
            return Err(Error::Config(format!(
                "key name '{t}' is ambiguous, use one of {dup:?}"
            )));
        }
        Err(Error::Config(format!("unknown key '{t}'")))
    }

    /// Sets one byte and logs it when it changes.
    pub fn put(&mut self, addr: usize, value: u8, why: impl Into<String>) {
        let old = self.b[addr];
        if old != value {
            self.b[addr] = value;
            self.log.push(Change {
                addr,
                old,
                new: value,
                why: why.into(),
            });
        }
    }

    /// Sets bytes and logs each one that changes, all with the same reason.
    pub fn put_bytes(&mut self, addr: usize, data: &[u8], why: &str) {
        for (i, &v) in data.iter().enumerate() {
            self.put(addr + i, v, why);
        }
    }

    fn remove_fn(&mut self, k: u8) -> Result<(), Error> {
        if self.capable(k) != Some(true) {
            return Err(Error::Config(format!(
                "{} has no Fn function to remove",
                self.name(k)
            )));
        }
        let e = self.checked_entry(k)?;
        let n = self.name(k).to_string();
        let ku = usize::from(k);
        for (km, layout) in KEYMAPS {
            let plain = self.b[layout_table(layout) + 2 * usize::from(e)];
            self.put(
                km + 2 * ku,
                plain,
                format!("keymap {km:#06x}: {n} back to its plain code (frees Fn entry {e})"),
            );
        }
        for bm in BITMAPS {
            let v = self.b[bm + (ku >> 3)] & !(1 << (k & 7));
            self.put(
                bm + (ku >> 3),
                v,
                format!("bitmap {bm:#06x}: {n} has no Fn function"),
            );
        }
        Ok(())
    }

    fn set_base(&mut self, k: u8, code: u8) -> Result<(), Error> {
        let n = self.name(k).to_string();
        let cn = code_name(Some(code));
        if self.capable(k) == Some(true) {
            let e = self.checked_entry(k)?;
            for (layout, t) in FN_TABLES {
                self.put(
                    t + 2 * usize::from(e),
                    code,
                    format!("{layout} Fn entry {e}: plain code of {n} = {cn}"),
                );
            }
        } else {
            for (km, _) in KEYMAPS {
                self.put(
                    km + 2 * usize::from(k),
                    code,
                    format!("keymap {km:#06x}: {n} = {cn}"),
                );
            }
        }
        Ok(())
    }

    fn free_entry(&self) -> Result<u8, Error> {
        // An entry is in use if any key has the Fn bit in either bitmap variant (W, X, C, R are Fn keys in
        // variant 0xc634 only, and their entries must stay theirs).
        let used: Vec<u8> = (0..128u8)
            .filter(|&k| BITMAPS.iter().any(|&bm| self.bit(bm, k)))
            .map(|k| self.entry(k))
            .collect();
        (0..FN_ENTRIES).find(|e| !used.contains(e)).ok_or_else(|| {
            Error::Config("all 39 Fn entries are in use; remove a Fn function first (value \"none\")".into())
        })
    }

    fn set_fn(&mut self, k: u8, code: u8) -> Result<(), Error> {
        let n = self.name(k).to_string();
        let cn = code_name(Some(code));
        let ku = usize::from(k);
        match self.capable(k) {
            None => {
                return Err(Error::Config(format!(
                    "{n} is Fn-capable in one bitmap variant only; not supported"
                )));
            }
            Some(true) => {
                let e = self.checked_entry(k)?;
                for (layout, t) in FN_TABLES {
                    self.put(
                        t + 2 * usize::from(e) + 1,
                        code,
                        format!("{layout} Fn entry {e}: Fn + {n} = {cn}"),
                    );
                }
                return Ok(());
            }
            Some(false) => {}
        }
        let e = self.free_entry()?;
        for (layout, t) in FN_TABLES {
            let mut plains: Vec<u8> = KEYMAPS
                .iter()
                .filter(|(_, l)| *l == layout)
                .map(|(km, _)| self.b[km + 2 * ku])
                .collect();
            plains.sort_unstable();
            plains.dedup();
            if plains.len() != 1 {
                return Err(Error::Config(format!(
                    "{n} has different plain codes in {layout} keymaps"
                )));
            }
            let eu = usize::from(e);
            self.put(
                t + 2 * eu,
                plains[0],
                format!("{layout} Fn entry {e}: plain code of {n}"),
            );
            self.put(
                t + 2 * eu + 1,
                code,
                format!("{layout} Fn entry {e}: Fn + {n} = {cn}"),
            );
        }
        for (km, _) in KEYMAPS {
            self.put(km + 2 * ku, e, format!("keymap {km:#06x}: {n} uses Fn entry {e}"));
        }
        for bm in BITMAPS {
            let v = self.b[bm + (ku >> 3)] | (1 << (k & 7));
            self.put(
                bm + (ku >> 3),
                v,
                format!("bitmap {bm:#06x}: {n} has a Fn function"),
            );
        }
        Ok(())
    }

    fn update_fn_position(&mut self) -> Result<u8, Error> {
        let fns: Vec<u8> = (0..128u8)
            .filter(|&k| self.normal(k, NAME_KEYMAP, "QWERTY") == FN_CODE)
            .collect();
        if fns.len() != 1 {
            let names: Vec<&str> = fns.iter().map(|&k| self.name(k)).collect();
            return Err(Error::Config(format!(
                "exactly one key must be Fn in [keymap], found {names:?}"
            )));
        }
        let k = fns[0];
        let why = format!(
            "boot code 0x0166: Fn position for macro recorder and Ctrl+Fn = {} (index {k})",
            self.name(k)
        );
        self.put(FN_POS_IMM, k, why);
        Ok(k)
    }

    /// Applies Fn-layer and keymap edits (in the given order; `None` in `fn_layer` removes the Fn function)
    /// and points the boot Fn position at the key whose plain code is Fn.
    pub fn apply(&mut self, fn_layer: &[(u8, Option<u8>)], keymap: &[(u8, u8)]) -> Result<u8, Error> {
        for &(k, c) in fn_layer {
            if c.is_none() {
                self.remove_fn(k)?;
            }
        }
        for &(k, c) in keymap {
            self.set_base(k, c)?;
        }
        for &(k, c) in fn_layer {
            if let Some(c) = c {
                self.set_fn(k, c)?;
            }
        }
        self.update_fn_position()
    }
}
