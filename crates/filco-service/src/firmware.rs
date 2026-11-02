//! The firmware draft: what the next build will contain, edited with undo and checked on every edit by patching
//! the stock image (the same code as the build, so an edit that is accepted here always builds). Also the list
//! of builds on disk and the pipeline run that makes them.
//!
//! Nothing here talks to a keyboard. Flashing is the user's: the app only starts the official updater.

use std::path::{Path, PathBuf};

use filco_firmware::v72::{FN_CODE, NAME_KEYMAP};
use filco_firmware::{Config, Firmware, HexFile, STOCK_HEX_NAME, STOCK_UPDATER_NAME};

/// File of the draft in the state folder, so closing the app keeps it.
pub const DRAFT_FILE: &str = "firmware_draft.toml";

#[derive(Debug, thiserror::Error)]
pub enum FwError {
    #[error("{path}: {msg}")]
    Io { path: PathBuf, msg: String },
    #[error(transparent)]
    Firmware(#[from] filco_firmware::Error),
    #[error(transparent)]
    Build(#[from] filco_fwbuild::Error),
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> FwError + '_ {
    move |e| FwError::Io {
        path: path.to_path_buf(),
        msg: e.to_string(),
    }
}

/// The stock V72 HEX and updater (shipped with the app), parsed once.
#[derive(Debug, Clone)]
pub struct Stock {
    dir: PathBuf,
    hex: String,
    fw: Firmware,
}

impl Stock {
    /// Loads `dir/<stock HEX>`; refuses anything that is not the stock image.
    pub fn load(dir: &Path) -> Result<Self, FwError> {
        let path = dir.join(STOCK_HEX_NAME);
        let hex = std::fs::read_to_string(&path).map_err(io(&path))?;
        if filco_firmware::v72::sha256_hex(hex.as_bytes()) != filco_firmware::v72::STOCK_HEX_SHA256 {
            return Err(filco_firmware::Error::NotStockHex.into());
        }
        let (_, img) = HexFile::parse(&hex).map_err(filco_firmware::Error::from)?;
        Ok(Stock {
            dir: dir.to_owned(),
            fw: Firmware::new(&img)?,
            hex,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn updater(&self) -> PathBuf {
        self.dir.join(STOCK_UPDATER_NAME)
    }

    pub fn firmware(&self) -> &Firmware {
        &self.fw
    }

    pub fn to_toml(&self, cfg: &Config) -> String {
        cfg.to_toml(&self.fw)
    }

    /// Patches the stock image with `cfg` (no emulator tests).
    pub fn build(&self, cfg: &Config) -> Result<filco_firmware::Build, filco_firmware::Error> {
        filco_firmware::build(&self.hex, &self.to_toml(cfg))
    }

    /// Reads a TOML configuration (the `custom.toml` format of the analysis repository).
    pub fn parse(&self, toml: &str) -> Result<Config, filco_firmware::Error> {
        Config::parse(toml, &self.fw)
    }

    /// A patched image as a [`Firmware`], to read its tables.
    fn patched(&self, image: Vec<u8>) -> Firmware {
        Firmware {
            stock: self.fw.stock.clone(),
            b: image,
            log: Vec::new(),
            names: self.fw.names.clone(),
        }
    }
}

/// The config a firmware info block describes. Entries come back sorted by key within each type (the order of
/// the block), which builds the same image as the original order: only the free Fn entry a new Fn key takes
/// could differ, and that is checked by comparing the checksum tag where it matters.
pub fn config_from_info(info: &filco_fwinfo::Info) -> Config {
    Config {
        name: info.name.clone(),
        info_block: true,
        fn_layer: info.fn_layer.iter().map(|f| (f.key, f.code)).collect(),
        keymap: info.keymap.iter().map(|k| (k.key, k.code)).collect(),
        tap_key: info.tap_key.map(|t| (t.key, t.tap, t.hold, t.term_ms)),
        tap_layers: info
            .tap_layers
            .iter()
            .map(|t| (t.key, t.tap, t.layer, t.term_ms))
            .collect(),
        layer_tap_hold: info.layer_tap_hold.is_some(),
    }
}

/// The config of a keyboard on stock firmware (or a patched one without an info block): nothing changed.
pub fn stock_config(name: &str) -> Config {
    Config {
        name: name.to_owned(),
        info_block: true,
        ..Config::default()
    }
}

/// What holding a tap-hold key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Sends this key while held (at most one such key, term 50-255 ms).
    Key(u8),
    /// Turns on layer 0-3 while held (at most two such keys, term 50-2000 ms).
    Layer(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapHold {
    pub tap: u8,
    pub hold: Hold,
    pub term_ms: u16,
}

/// One edit of the draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FwEdit {
    /// Fn + key sends `code`; `None` removes the key's Fn function.
    SetFn { key: u8, code: Option<u8> },
    /// Default code of the key, on every layer and layout.
    SetDefault { key: u8, code: u8 },
    /// Makes the key tap-hold, or plain again with `None`.
    SetTapHold { key: u8, tap_hold: Option<TapHold> },
    /// Everything of one key back to what the keyboard holds.
    KeyToBase { key: u8 },
    /// Everything of one key back to stock V72.
    KeyToStock { key: u8 },
    /// Build name (1-32 letters, digits, `_` or `-`).
    SetName(String),
    /// Tap-hold keys set per layer in the layer slots (type 8) instead of the global ones of the draft; turning
    /// it on drops the global tap-hold keys, which it replaces.
    SetLayerTapHold(bool),
}

fn tap_hold_of(cfg: &Config, k: u8) -> Option<TapHold> {
    if let Some((key, tap, hold, term_ms)) = cfg.tap_key
        && key == k
    {
        return Some(TapHold {
            tap,
            hold: Hold::Key(hold),
            term_ms,
        });
    }
    cfg.tap_layers
        .iter()
        .find(|t| t.0 == k)
        .map(|&(_, tap, layer, term_ms)| TapHold {
            tap,
            hold: Hold::Layer(layer),
            term_ms,
        })
}

fn set_tap_hold(cfg: &mut Config, k: u8, th: Option<TapHold>) {
    if cfg.tap_key.is_some_and(|t| t.0 == k) {
        cfg.tap_key = None;
    }
    let at = cfg.tap_layers.iter().position(|t| t.0 == k);
    match th {
        None => {
            if let Some(i) = at {
                cfg.tap_layers.remove(i);
            }
        }
        Some(TapHold {
            tap,
            hold: Hold::Key(hold),
            term_ms,
        }) => {
            if let Some(i) = at {
                cfg.tap_layers.remove(i);
            }
            cfg.tap_key = Some((k, tap, hold, term_ms));
        }
        Some(TapHold {
            tap,
            hold: Hold::Layer(layer),
            term_ms,
        }) => {
            let v = (k, tap, layer, term_ms);
            match at {
                Some(i) => cfg.tap_layers[i] = v,
                None => cfg.tap_layers.push(v),
            }
        }
    }
}

fn upsert<V: Copy>(list: &mut Vec<(u8, V)>, k: u8, v: Option<V>) {
    let at = list.iter().position(|e| e.0 == k);
    match (at, v) {
        (Some(i), Some(v)) => list[i].1 = v,
        (None, Some(v)) => list.push((k, v)),
        (Some(i), None) => {
            list.remove(i);
        }
        (None, None) => {}
    }
}

/// Copies everything `from` says about key `k` into `to` (absent in `from` means stock).
fn copy_key(to: &mut Config, from: &Config, k: u8) {
    upsert(
        &mut to.fn_layer,
        k,
        from.fn_layer.iter().find(|e| e.0 == k).map(|e| e.1),
    );
    upsert(
        &mut to.keymap,
        k,
        from.keymap.iter().find(|e| e.0 == k).map(|e| e.1),
    );
    set_tap_hold(to, k, tap_hold_of(from, k));
}

/// Applies `edit` to a copy of `cfg`. `base` is what the keyboard holds.
fn edited(cfg: &Config, base: &Config, stock: &Stock, edit: &FwEdit) -> Config {
    let mut c = cfg.clone();
    let fw = stock.firmware();
    match *edit {
        FwEdit::SetFn { key, code } => {
            // Setting the stock value again is no edit at all.
            let stock_fn = fw.fn_output(key);
            let v = if code == stock_fn { None } else { Some(code) };
            upsert(&mut c.fn_layer, key, v);
        }
        FwEdit::SetDefault { key, code } => {
            let v = (code != fw.normal(key, NAME_KEYMAP, "QWERTY")).then_some(code);
            upsert(&mut c.keymap, key, v);
        }
        FwEdit::SetTapHold { key, tap_hold } => set_tap_hold(&mut c, key, tap_hold),
        FwEdit::KeyToBase { key } => copy_key(&mut c, base, key),
        FwEdit::KeyToStock { key } => copy_key(&mut c, &stock_config(""), key),
        FwEdit::SetName(ref n) => c.name.clone_from(n),
        FwEdit::SetLayerTapHold(on) => {
            c.layer_tap_hold = on;
            if on {
                c.tap_key = None;
                c.tap_layers.clear();
            }
        }
    }
    c
}

/// A key of the draft as the UI shows it, read from the patched image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FwKey {
    pub index: u8,
    /// Fn + key now; `None` when the key has no Fn function.
    pub fn_code: Option<u8>,
    /// Fn + key on the keyboard (base) and in stock V72.
    pub fn_base: Option<u8>,
    pub fn_stock: Option<u8>,
    /// Default code now, on the keyboard and in stock.
    pub default: u8,
    pub default_base: u8,
    pub default_stock: u8,
    pub tap_hold: Option<TapHold>,
    pub tap_hold_base: Option<TapHold>,
    /// Why Fn + key cannot be edited: `layout_specific` (Fn function in one bitmap variant only: W, R, X, C),
    /// `fn_key`, `tap_hold`.
    pub fn_locked: Option<&'static str>,
    /// Why the key cannot be a global tap-hold key: `layer_tap` (tap-hold is set per layer on the layer page),
    /// `has_fn` (remove its Fn function first), `fn_key`.
    pub tap_hold_locked: Option<&'static str>,
}

impl FwKey {
    /// Differs from what the keyboard holds.
    pub fn changed(&self) -> bool {
        self.fn_code != self.fn_base
            || self.default != self.default_base
            || self.tap_hold != self.tap_hold_base
    }

    /// Differs from stock V72.
    pub fn custom(&self) -> bool {
        self.fn_code != self.fn_stock || self.default != self.default_stock || self.tap_hold.is_some()
    }
}

/// The tables of one patched image, read once per edit.
#[derive(Debug, Clone)]
struct Tables {
    fw: Firmware,
}

impl Tables {
    fn of(stock: &Stock, cfg: &Config) -> Result<Self, filco_firmware::Error> {
        Ok(Tables {
            fw: stock.patched(stock.build(cfg)?.image),
        })
    }

    fn fn_code(&self, k: u8) -> Option<u8> {
        // W, R, X, C (Fn in one bitmap variant) are drawn with the variant the keyboard uses by default.
        match self.fw.capable(k) {
            Some(true) => self.fw.fn_output(k),
            Some(false) => None,
            None => {
                Some(self.fw.b[filco_firmware::v72::FN_TABLES[0].1 + 2 * usize::from(self.fw.entry(k)) + 1])
            }
        }
    }

    fn default(&self, k: u8) -> u8 {
        self.fw.normal(k, NAME_KEYMAP, "QWERTY")
    }

    /// Fn entries in use: keys with the Fn bit in either bitmap variant.
    fn fn_slots_used(&self) -> usize {
        (0..128u8)
            .filter(|&k| {
                filco_firmware::v72::BITMAPS
                    .iter()
                    .any(|&bm| (self.fw.b[bm + usize::from(k >> 3)] >> (k & 7)) & 1 == 1)
            })
            .count()
    }
}

/// The draft with undo. `base` is the config of the firmware on the keyboard.
#[derive(Debug, Clone)]
pub struct FwDraft {
    base: Config,
    working: Config,
    undo: Vec<Config>,
    redo: Vec<Config>,
    stock_t: Tables,
    base_t: Tables,
    working_t: Tables,
}

impl FwDraft {
    /// Starts from what the keyboard holds. `base` must build (a config read from an info block does).
    pub fn new(stock: &Stock, base: Config) -> Result<Self, filco_firmware::Error> {
        let stock_t = Tables::of(stock, &stock_config("stock"))?;
        let base_t = Tables::of(stock, &base)?;
        Ok(FwDraft {
            working: base.clone(),
            working_t: base_t.clone(),
            base,
            undo: Vec::new(),
            redo: Vec::new(),
            stock_t,
            base_t,
        })
    }

    pub fn base(&self) -> &Config {
        &self.base
    }

    pub fn working(&self) -> &Config {
        &self.working
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn set(&mut self, stock: &Stock, cfg: Config) -> Result<(), filco_firmware::Error> {
        let t = Tables::of(stock, &cfg)?;
        self.working = cfg;
        self.working_t = t;
        Ok(())
    }

    /// Applies an edit as one undoable step. An edit the build would refuse is refused here, with the build's
    /// message, and leaves the draft as it was.
    pub fn apply(&mut self, stock: &Stock, edit: &FwEdit) -> Result<(), filco_firmware::Error> {
        let cfg = edited(&self.working, &self.base, stock, edit);
        self.replace(stock, cfg)
    }

    /// Replaces the whole draft (import, autosaved draft) as one undoable step.
    pub fn replace(&mut self, stock: &Stock, mut cfg: Config) -> Result<(), filco_firmware::Error> {
        cfg.info_block = true;
        if cfg == self.working {
            return Ok(());
        }
        let before = self.working.clone();
        self.set(stock, cfg)?;
        self.undo.push(before);
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self, stock: &Stock) {
        if let Some(c) = self.undo.pop() {
            let now = self.working.clone();
            if self.set(stock, c).is_ok() {
                self.redo.push(now);
            }
        }
    }

    pub fn redo(&mut self, stock: &Stock) {
        if let Some(c) = self.redo.pop() {
            let now = self.working.clone();
            if self.set(stock, c).is_ok() {
                self.undo.push(now);
            }
        }
    }

    /// Back to what the keyboard holds (undoable).
    pub fn revert(&mut self, stock: &Stock) {
        let _ = self.replace(stock, self.base.clone());
    }

    /// The keyboard now holds `base` (after a flash): the draft is measured against it from now on.
    pub fn rebase(&mut self, stock: &Stock, base: Config) -> Result<(), filco_firmware::Error> {
        self.base_t = Tables::of(stock, &base)?;
        self.base = base;
        Ok(())
    }

    pub fn key(&self, k: u8, fn_position: u8) -> FwKey {
        let fn_code = self.working_t.fn_code(k);
        let tap_hold = tap_hold_of(&self.working, k);
        let capable = self.working_t.fw.capable(k);
        let is_fn_key = k == fn_position || self.working_t.default(k) == FN_CODE;
        FwKey {
            index: k,
            fn_code,
            fn_base: self.base_t.fn_code(k),
            fn_stock: self.stock_t.fn_code(k),
            default: self.working_t.default(k),
            default_base: self.base_t.default(k),
            default_stock: self.stock_t.default(k),
            tap_hold,
            tap_hold_base: tap_hold_of(&self.base, k),
            fn_locked: if is_fn_key {
                Some("fn_key")
            } else if capable.is_none() {
                Some("layout_specific")
            } else if tap_hold.is_some() {
                Some("tap_hold")
            } else {
                None
            },
            tap_hold_locked: if self.working.layer_tap_hold {
                Some("layer_tap")
            } else if is_fn_key {
                Some("fn_key")
            } else if capable != Some(false) {
                Some("has_fn")
            } else {
                None
            },
        }
    }

    /// Index of the Fn key in the draft.
    pub fn fn_position(&self) -> u8 {
        self.working_t.fw.fn_position()
    }

    pub fn fn_slots_used(&self) -> usize {
        self.working_t.fn_slots_used()
    }

    /// Keys whose Fn function, default code or tap-hold differ from the keyboard.
    pub fn changed_keys(&self) -> Vec<u8> {
        let p = self.fn_position();
        (0..128u8)
            .filter(|&k| self.stock_t.fw.names.contains_key(&k) && self.key(k, p).changed())
            .collect()
    }

    /// Layer tap-hold in the draft and on the keyboard.
    pub fn layer_tap_hold(&self) -> (bool, bool) {
        (self.working.layer_tap_hold, self.base.layer_tap_hold)
    }

    /// Whether anything differs from the keyboard, the name included.
    pub fn is_changed(&self) -> bool {
        self.working != self.base
    }
}

/// Loads the autosaved draft from the state folder, if there is one that still builds.
pub fn load_saved(state: &Path, stock: &Stock) -> Option<Config> {
    let text = std::fs::read_to_string(state.join(DRAFT_FILE)).ok()?;
    let cfg = stock.parse(&text).ok()?;
    stock.build(&cfg).ok()?;
    Some(cfg)
}

/// Saves the draft, or removes the file when the draft equals the keyboard.
pub fn save(state: &Path, stock: &Stock, draft: &FwDraft) -> Result<(), FwError> {
    let path = state.join(DRAFT_FILE);
    if draft.is_changed() {
        std::fs::create_dir_all(state).map_err(io(state))?;
        std::fs::write(&path, stock.to_toml(draft.working())).map_err(io(&path))
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(&path)(e)),
            _ => Ok(()),
        }
    }
}

/// A build on disk, under `<root>/<name>/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub name: String,
    pub dir: PathBuf,
    /// Checksum tag, from the HEX file name.
    pub tag: Option<u16>,
    /// The updater, present only when every test passed.
    pub updater: Option<PathBuf>,
    /// Seconds since the epoch when the report was written.
    pub modified: u64,
}

fn tag_of_hex(name: &str) -> Option<u16> {
    let start = name.find("[0x")? + 3;
    let end = start + name[start..].find(']')?;
    u16::from_str_radix(&name[start..end], 16).ok()
}

/// Builds under `root`, newest first.
pub fn list_builds(root: &Path) -> Vec<Built> {
    let Ok(dirs) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<Built> = dirs
        .flatten()
        .filter(|d| d.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|d| {
            let dir = d.path();
            let report = std::fs::metadata(dir.join("TEST_REPORT.txt")).ok()?;
            let files: Vec<String> = std::fs::read_dir(&dir)
                .ok()?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            let name = d.file_name().to_string_lossy().into_owned();
            Some(Built {
                tag: files
                    .iter()
                    .find(|f| f.to_ascii_lowercase().ends_with(".hex"))
                    .and_then(|f| tag_of_hex(f)),
                updater: files
                    .iter()
                    .find(|f| f.to_ascii_lowercase().ends_with(".exe"))
                    .map(|f| dir.join(f)),
                modified: report
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs()),
                name,
                dir,
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Runs the whole pipeline (patch, emulator tests, updater when they pass) for `cfg` into `root/<name>/`. The
/// config is kept as `root/../firmware-configs/<name>.toml`, the path the reports print.
pub fn run_build(
    stock: &Stock,
    cfg: &Config,
    root: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<filco_fwbuild::Outcome, FwError> {
    let configs = root
        .parent()
        .map_or_else(|| root.join("configs"), |p| p.join("firmware-configs"));
    std::fs::create_dir_all(&configs).map_err(io(&configs))?;
    let path = configs.join(format!("{}.toml", cfg.name));
    std::fs::write(&path, stock.to_toml(cfg)).map_err(io(&path))?;
    Ok(filco_fwbuild::run(stock.dir(), &path, root, progress)?)
}

/// One step of a build as the dialog shows it, counted from the report lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// `image`, `keys`, `fn`, `layers`, `tap_hold`, `info`, `stack`, `updater`.
    pub id: &'static str,
    pub ok: u32,
    pub fail: u32,
}

const STEP_IDS: [&str; 8] = [
    "image", "keys", "fn", "layers", "tap_hold", "info", "stack", "updater",
];

/// Which step a report section belongs to, by its title.
fn section(title: &str) -> Option<&'static str> {
    let t = title.trim_start();
    Some(if t.starts_with("Build:") {
        "image"
    } else if t.starts_with("Every key") {
        "keys"
    } else if t.starts_with("Fn keys") {
        "fn"
    } else if t.starts_with("Layer switching")
        || t.starts_with("Fn layer with")
        || t.starts_with("Two layer keys")
    {
        "layers"
    } else if t.starts_with("Tap-hold") {
        "tap_hold"
    } else if t.starts_with("Info block") {
        "info"
    } else if t.starts_with("Stack depth") {
        "stack"
    } else if t.starts_with("ISPTitle") || t.starts_with("All tests passed") || t.starts_with("TESTS FAILED")
    {
        "updater"
    } else {
        return None;
    })
}

/// Steps seen so far in `lines` (the report as it streams), in pipeline order. A line ending in `OK` (or
/// holding ` OK `) counts as passed, one holding `FAIL` as failed; the `image` and `updater` steps count one
/// when they finish.
pub fn report_steps(lines: &[String]) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut cur: Option<usize> = None;
    for l in lines {
        let header = !l.starts_with(' ') || l.trim_start().starts_with("ISPTitle");
        if header && let Some(id) = section(l) {
            let at = steps.iter().position(|s| s.id == id).unwrap_or_else(|| {
                steps.push(Step { id, ok: 0, fail: 0 });
                steps.len() - 1
            });
            cur = Some(at);
            if id == "image" {
                steps[at].ok = 1;
            }
            if l.starts_with("All tests passed") {
                steps[at].ok = 1;
            } else if l.starts_with("TESTS FAILED") {
                steps[at].fail = 1;
            }
            continue;
        }
        let Some(at) = cur else { continue };
        if matches!(steps[at].id, "updater" | "image") {
            continue;
        }
        if l.contains("FAIL") || l.contains("emulator stopped") {
            steps[at].fail += 1;
        } else if l.trim_end().ends_with(" OK") || l.contains(" OK ") || l.contains(" OK (") {
            steps[at].ok += 1;
        }
    }
    steps.sort_by_key(|s| STEP_IDS.iter().position(|i| *i == s.id));
    steps
}

#[cfg(test)]
#[allow(
    clippy::many_single_char_names,
    reason = "one letter per key, as on the keycaps"
)]
mod tests {
    use super::*;

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn stock() -> Stock {
        Stock::load(&repo().join("firmware")).unwrap()
    }

    fn fixture_info(name: &str) -> filco_fwinfo::Info {
        let img = std::fs::read(
            repo()
                .join("tests/fixtures/firmware")
                .join(name)
                .join("image.bin"),
        )
        .unwrap();
        filco_fwinfo::from_image(&img).unwrap()
    }

    fn idx(s: &Stock, name: &str) -> u8 {
        s.firmware().key(name).unwrap()
    }

    #[test]
    fn report_steps_count_the_golden_report() {
        let text = std::fs::read_to_string(
            repo().join("tests/fixtures/firmware/HJKL_CapsSpace2_Info/TEST_REPORT.txt"),
        )
        .unwrap();
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let got: Vec<(&str, u32, u32)> = report_steps(&lines)
            .iter()
            .map(|s| (s.id, s.ok, s.fail))
            .collect();
        assert_eq!(
            got,
            vec![
                ("image", 1, 0),
                ("keys", 16, 0),
                ("fn", 1, 0),
                ("layers", 19, 0),
                ("tap_hold", 36, 0),
                ("info", 11, 0),
                ("stack", 9, 0),
                ("updater", 1, 0)
            ]
        );
        let upto = lines
            .iter()
            .position(|l| l.starts_with("Layer switching"))
            .unwrap()
            + 1;
        let half: Vec<&str> = report_steps(&lines[..upto]).iter().map(|s| s.id).collect();
        assert_eq!(
            half,
            vec!["image", "keys", "fn", "layers"],
            "steps appear as the report streams"
        );
    }

    #[test]
    fn the_info_block_rebuilds_the_same_firmware() {
        let s = stock();
        let cfg = config_from_info(&fixture_info("HJKL_CapsSpace2_Info"));
        let b = s.build(&cfg).unwrap();
        assert_eq!(b.tag(), 0x1321, "same checksum as the image on the keyboard");
        let img =
            std::fs::read(repo().join("tests/fixtures/firmware/HJKL_CapsSpace2_Info/image.bin")).unwrap();
        assert!(b.image == img, "image differs");
    }

    #[test]
    fn keys_read_from_the_patched_image() {
        let s = stock();
        let d = FwDraft::new(&s, config_from_info(&fixture_info("HJKL_CapsSpace2_Info"))).unwrap();
        let p = d.fn_position();
        let h = d.key(idx(&s, "H"), p);
        assert_eq!(h.fn_code, Some(0x50), "Fn + H = Left");
        assert_eq!(h.fn_stock, None);
        assert!(h.custom() && !h.changed());
        let i = d.key(idx(&s, "I"), p);
        assert_eq!((i.fn_code, i.fn_stock), (None, Some(0x52)), "Fn + I removed");
        let s_key = d.key(idx(&s, "S"), p);
        assert_eq!(s_key.fn_code, Some(0xF9), "mouse left");
        let w = d.key(idx(&s, "W"), p);
        assert_eq!((w.fn_code, w.fn_locked), (Some(0xF7), Some("layout_specific")));
        let caps = d.key(idx(&s, "CapsLock"), p);
        assert_eq!(
            caps.tap_hold,
            Some(TapHold {
                tap: 0x29,
                hold: Hold::Key(0xE0),
                term_ms: 200
            })
        );
        assert_eq!(caps.fn_locked, Some("tap_hold"));
        assert_eq!(d.key(p, p).fn_locked, Some("fn_key"));
        assert_eq!(d.fn_slots_used(), 39 - 1 + 1, "I freed one entry, H took it back");
    }

    #[test]
    fn edits_undo_and_refusals() {
        let s = stock();
        let mut d = FwDraft::new(&s, config_from_info(&fixture_info("HJKL_CapsSpace2_Info"))).unwrap();
        let p = d.fn_position();
        let o = idx(&s, "O");
        d.apply(
            &s,
            &FwEdit::SetFn {
                key: o,
                code: Some(0x4B),
            },
        )
        .unwrap();
        assert_eq!(d.key(o, p).fn_code, Some(0x4B));
        assert_eq!(d.changed_keys(), vec![o]);
        // Every Fn entry is in use now: a new Fn key is refused with the build's message.
        let q = idx(&s, "Q");
        let e = d
            .apply(
                &s,
                &FwEdit::SetFn {
                    key: q,
                    code: Some(0x04),
                },
            )
            .unwrap_err();
        assert!(e.to_string().contains("39 Fn entries"), "{e}");
        assert_eq!(d.changed_keys(), vec![o], "a refused edit changes nothing");
        // Two layer holds already: a third is refused.
        let tab = idx(&s, "Tab");
        d.apply(&s, &FwEdit::SetFn { key: tab, code: None }).unwrap();
        let third = FwEdit::SetTapHold {
            key: tab,
            tap_hold: Some(TapHold {
                tap: 0x2B,
                hold: Hold::Layer(1),
                term_ms: 300,
            }),
        };
        assert!(d.apply(&s, &third).is_err());
        d.undo(&s);
        d.undo(&s);
        assert!(!d.is_changed());
        d.redo(&s);
        assert_eq!(d.changed_keys(), vec![o]);
        d.apply(&s, &FwEdit::KeyToBase { key: o }).unwrap();
        assert!(!d.is_changed());
        d.apply(&s, &FwEdit::KeyToStock { key: idx(&s, "H") }).unwrap();
        assert_eq!(d.key(idx(&s, "H"), p).fn_code, None);
        assert!(d.apply(&s, &FwEdit::SetName("bad name".into())).is_err());
    }

    #[test]
    fn layer_tap_hold_replaces_the_firmware_tap_holds() {
        let s = stock();
        let mut d = FwDraft::new(&s, config_from_info(&fixture_info("HJKL_CapsSpace2_Info"))).unwrap();
        let p = d.fn_position();
        let caps = idx(&s, "CapsLock");
        d.apply(&s, &FwEdit::SetLayerTapHold(true)).unwrap();
        assert_eq!(d.layer_tap_hold(), (true, false));
        assert!(d.working().tap_key.is_none() && d.working().tap_layers.is_empty());
        let k = d.key(caps, p);
        assert_eq!((k.tap_hold, k.tap_hold_locked), (None, Some("layer_tap")));
        let older = FwEdit::SetTapHold {
            key: caps,
            tap_hold: Some(TapHold {
                tap: 0x29,
                hold: Hold::Key(0xE0),
                term_ms: 200,
            }),
        };
        assert!(
            d.apply(&s, &older).is_err(),
            "no firmware tap-hold next to layer tap-hold"
        );
        d.undo(&s);
        assert_eq!(d.layer_tap_hold(), (false, false));
        assert!(
            d.key(caps, p).tap_hold.is_some(),
            "undo brings the firmware tap-hold back"
        );
    }

    #[test]
    fn draft_survives_a_restart_and_builds_are_listed() {
        let s = stock();
        let tmp = tempfile::tempdir().unwrap();
        let mut d = FwDraft::new(&s, stock_config("custom")).unwrap();
        save(tmp.path(), &s, &d).unwrap();
        assert!(
            load_saved(tmp.path(), &s).is_none(),
            "nothing saved when unchanged"
        );
        let caps = idx(&s, "CapsLock");
        d.apply(
            &s,
            &FwEdit::SetTapHold {
                key: caps,
                tap_hold: Some(TapHold {
                    tap: 0x29,
                    hold: Hold::Key(0xE0),
                    term_ms: 180,
                }),
            },
        )
        .unwrap();
        save(tmp.path(), &s, &d).unwrap();
        assert_eq!(load_saved(tmp.path(), &s).as_ref(), Some(d.working()));

        let root = tmp.path().join("firmware");
        let out = run_build(&s, d.working(), &root, &mut |_| {}).unwrap();
        assert!(out.passed, "{}", out.report.join("\n"));
        let list = list_builds(&root);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "custom");
        assert_eq!(list[0].tag, Some(out.tag));
        assert_eq!(list[0].updater, out.updater);
        assert!(tmp.path().join("firmware-configs/custom.toml").is_file());
    }
}
