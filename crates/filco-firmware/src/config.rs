//! Build configuration, read from the TOML format of the analysis repository (`build.py`, `custom.toml`):
//!
//! ```toml
//! name = "HJKL_CapsSpace2_Info"   # 1-32 letters, digits, '_' or '-'
//! info_block = true               # optional: add the info block and HID command 0x30
//! layer_tap_hold = true           # optional: tap-hold keys set per layer in the layer slots (type 8);
//!                                 # instead of [[tap_hold]]
//! [fn_layer]                      # Fn + key = code; "none" removes the key's Fn function
//! H = "Left"
//! [keymap]                        # default code of a key, every layer and layout
//! [[tap_hold]]                    # at most one with hold, at most two with hold_layer
//! key = "CapsLock"
//! tap = "Esc"
//! hold = "LCtrl"                  # or hold_layer = 1-4 (as on the LED)
//! term_ms = 200                   # hold: 50-255 (default 200); hold_layer: 50-2000 (default 300)
//! ```
//!
//! Keys are names as the stock firmware gives them ("H", "Space#77") or "#index". Entries keep the order of
//! the file, which decides the order of the edits and of the patch log.

use toml::{Table, Value};

use crate::Error;
use crate::v72::{Firmware, code_name, parse_code};

/// A parsed configuration, with keys resolved to indices.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub name: String,
    pub info_block: bool,
    /// (key, Fn + key code); `None` removes the Fn function.
    pub fn_layer: Vec<(u8, Option<u8>)>,
    /// (key, default code)
    pub keymap: Vec<(u8, u8)>,
    /// Key hold: (key, tap, hold, term_ms).
    pub tap_key: Option<(u8, u8, u8, u16)>,
    /// Layer holds: (key, tap, layer 0-3, term_ms).
    pub tap_layers: Vec<(u8, u8, u8, u16)>,
    /// Tap-hold read from the layer slots of type 8 (`layertap.rs`), every key and layer; excludes `tap_hold`.
    pub layer_tap_hold: bool,
}

pub const MAX_LAYER_KEYS: usize = crate::taphold::MAX_LAYER_KEYS;

fn bad<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error::Config(msg.into()))
}

fn upsert<V>(list: &mut Vec<(u8, V)>, k: u8, v: V) {
    match list.iter_mut().find(|e| e.0 == k) {
        Some(e) => e.1 = v,
        None => list.push((k, v)),
    }
}

fn text(v: &Value, what: &str) -> Result<String, Error> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Integer(i) => Ok(i.to_string()),
        _ => bad(format!("{what}: expected a key or code name")),
    }
}

fn code(v: &Value, what: &str) -> Result<u8, Error> {
    parse_code(&text(v, what)?, false)?.map_or_else(|| bad(format!("{what}: 'none' not allowed")), Ok)
}

fn is_valid_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl Config {
    /// Parses and checks a configuration against the stock firmware's key names.
    pub fn parse(src: &str, fw: &Firmware) -> Result<Self, Error> {
        let cfg: Table = src
            .parse()
            .map_err(|e: toml::de::Error| Error::Config(e.to_string()))?;
        let mut unknown: Vec<&String> = cfg
            .keys()
            .filter(|k| {
                ![
                    "name",
                    "info_block",
                    "fn_layer",
                    "keymap",
                    "tap_hold",
                    "layer_tap_hold",
                ]
                .contains(&k.as_str())
            })
            .collect();
        unknown.sort();
        if !unknown.is_empty() {
            return bad(format!("unknown settings {unknown:?}"));
        }
        let name = cfg.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        if !is_valid_name(&name) {
            return bad("name must be 1-32 letters, digits, '_' or '-'");
        }
        let mut c = Self {
            name,
            ..Self::default()
        };
        let table = |key: &str| -> Result<Option<&Table>, Error> {
            match cfg.get(key) {
                None => Ok(None),
                Some(Value::Table(t)) => Ok(Some(t)),
                Some(_) => bad(format!("[{key}] must be a table")),
            }
        };
        if let Some(t) = table("fn_layer")? {
            for (k, v) in t {
                upsert(&mut c.fn_layer, fw.key(k)?, parse_code(&text(v, k)?, true)?);
            }
        }
        if let Some(t) = table("keymap")? {
            for (k, v) in t {
                upsert(&mut c.keymap, fw.key(k)?, code(v, k)?);
            }
        }
        let entries: Vec<&Table> = match cfg.get("tap_hold") {
            None => Vec::new(),
            Some(Value::Table(t)) => vec![t],
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_table()
                        .map_or_else(|| bad("[[tap_hold]] entries must be tables"), Ok)
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return bad("[[tap_hold]] must be a table array"),
        };
        for th in entries {
            c.add_tap_hold(th, fw)?;
        }
        let mut keys: Vec<u8> = c
            .tap_key
            .iter()
            .map(|s| s.0)
            .chain(c.tap_layers.iter().map(|s| s.0))
            .collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        if keys.len() != n {
            return bad("the [[tap_hold]] keys must differ");
        }
        c.info_block = match cfg.get("info_block") {
            None => false,
            Some(Value::Boolean(b)) => *b,
            Some(_) => return bad("info_block must be true or false"),
        };
        c.layer_tap_hold = match cfg.get("layer_tap_hold") {
            None => false,
            Some(Value::Boolean(b)) => *b,
            Some(_) => return bad("layer_tap_hold must be true or false"),
        };
        if c.layer_tap_hold && c.has_tap_hold() {
            return bad("layer_tap_hold replaces [[tap_hold]]: use one of them");
        }
        if c.fn_layer.is_empty()
            && c.keymap.is_empty()
            && !c.has_tap_hold()
            && !c.layer_tap_hold
            && !c.info_block
        {
            return bad("nothing to change");
        }
        Ok(c)
    }

    fn add_tap_hold(&mut self, th: &Table, fw: &Firmware) -> Result<(), Error> {
        let mut keys: Vec<&String> = th.keys().collect();
        keys.sort();
        let extra = th
            .keys()
            .any(|k| !["key", "tap", "hold", "hold_layer", "term_ms"].contains(&k.as_str()));
        if extra
            || !th.contains_key("key")
            || !th.contains_key("tap")
            || th.contains_key("hold") == th.contains_key("hold_layer")
        {
            return bad(format!(
                "[[tap_hold]] needs key, tap, one of hold / hold_layer, optionally term_ms (got {keys:?})"
            ));
        }
        let k = fw.key(&text(&th["key"], "key")?)?;
        let tap = code(&th["tap"], "tap")?;
        let term = |default: i64| match th.get("term_ms") {
            None => Some(default),
            Some(Value::Integer(i)) => Some(*i),
            Some(_) => None,
        };
        if let Some(hold) = th.get("hold") {
            let term = term(200).filter(|t| (50..=255).contains(t));
            let Some(term) = term else {
                return bad("[[tap_hold]] with hold: term_ms must be 50-255");
            };
            if self.tap_key.is_some() {
                return bad("at most one [[tap_hold]] with hold");
            }
            self.tap_key = Some((k, tap, code(hold, "hold")?, u16::try_from(term).unwrap_or(0)));
        } else {
            let term = term(300).filter(|t| (50..=2000).contains(t));
            let Some(term) = term else {
                return bad("[[tap_hold]] with hold_layer: term_ms must be 50-2000");
            };
            let layer = th["hold_layer"].as_integer().filter(|l| (1..=4).contains(l));
            let Some(layer) = layer else {
                return bad("hold_layer must be 1-4 (as on the keyboard LED)");
            };
            if self.tap_layers.len() == MAX_LAYER_KEYS {
                return bad(format!("at most {MAX_LAYER_KEYS} [[tap_hold]] with hold_layer"));
            }
            let layer = u8::try_from(layer - 1).unwrap_or(0);
            self.tap_layers
                .push((k, tap, layer, u16::try_from(term).unwrap_or(0)));
        }
        Ok(())
    }

    /// Whether any tap-hold key is configured.
    pub fn has_tap_hold(&self) -> bool {
        self.tap_key.is_some() || !self.tap_layers.is_empty()
    }

    /// The configuration as TOML that [`Config::parse`] reads back to the same value: keys by their stock names,
    /// codes by name (`0xNN` when unnamed), entries in the order of the lists.
    pub fn to_toml(&self, fw: &Firmware) -> String {
        let key = |k: u8| fw.names.get(&k).cloned().unwrap_or_else(|| format!("#{k}"));
        let code = |c: u8| Value::String(code_name(Some(c)));
        let mut t = Table::new();
        t.insert("name".into(), Value::String(self.name.clone()));
        if self.info_block {
            t.insert("info_block".into(), Value::Boolean(true));
        }
        if self.layer_tap_hold {
            t.insert("layer_tap_hold".into(), Value::Boolean(true));
        }
        if !self.fn_layer.is_empty() {
            let fl: Table = self
                .fn_layer
                .iter()
                .map(|&(k, c)| (key(k), c.map_or_else(|| Value::String("none".into()), code)))
                .collect();
            t.insert("fn_layer".into(), Value::Table(fl));
        }
        if !self.keymap.is_empty() {
            let km: Table = self.keymap.iter().map(|&(k, c)| (key(k), code(c))).collect();
            t.insert("keymap".into(), Value::Table(km));
        }
        let mut holds = Vec::new();
        if let Some((k, tap, hold, term)) = self.tap_key {
            let mut h = Table::new();
            h.insert("key".into(), Value::String(key(k)));
            h.insert("tap".into(), code(tap));
            h.insert("hold".into(), code(hold));
            h.insert("term_ms".into(), Value::Integer(i64::from(term)));
            holds.push(Value::Table(h));
        }
        for &(k, tap, layer, term) in &self.tap_layers {
            let mut h = Table::new();
            h.insert("key".into(), Value::String(key(k)));
            h.insert("tap".into(), code(tap));
            h.insert("hold_layer".into(), Value::Integer(i64::from(layer) + 1));
            h.insert("term_ms".into(), Value::Integer(i64::from(term)));
            holds.push(Value::Table(h));
        }
        if !holds.is_empty() {
            t.insert("tap_hold".into(), Value::Array(holds));
        }
        toml::to_string(&t).unwrap_or_default()
    }
}
