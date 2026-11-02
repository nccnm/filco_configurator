//! Firmware mode of the [`App`]: the draft, the build (run outside the app lock, see [`BuildJob`]) and the
//! flash flow. The app never flashes: it starts the official updater when the user asks, waits for the keyboard
//! to come back and checks what it runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use filco_firmware::Config;
use filco_service::firmware::{
    self as fw, Built, FwDraft, FwEdit, FwKey, Hold, Stock, TapHold, config_from_info, stock_config,
};
use filco_service::guard::ensure_assist_closed;
use filco_service::store::timestamp_now;

use super::{App, guard_error};
use crate::view::{
    ErrorView, FwBuildView, FwBuiltView, FwCodeView, FwEditInput, FwFlashView, FwHoldView, FwKeyView,
    FwStepView, FwTapHoldView, FwView, UiLang,
};

/// Starts the updater for a build (`dir` holds it). The real one runs the exe; tests and the E2E build simulate
/// a flash instead.
pub type Launcher = Box<dyn Fn(&Path) -> Result<(), String> + Send>;

/// Runs the updater exe of a build folder. Only ever called when the user pressed the button.
pub fn launch_updater(dir: &Path) -> Result<(), String> {
    let exe = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")))
        .ok_or_else(|| "no updater in the build folder".to_owned())?;
    std::process::Command::new(&exe)
        .current_dir(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A finished build of this session.
#[derive(Debug, Clone)]
pub struct Finished {
    config: Config,
    tag: Option<u16>,
    dir: Option<PathBuf>,
    passed: bool,
    has_updater: bool,
    report: Vec<String>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum BuildState {
    Running {
        name: String,
        generation: u64,
        lines: Vec<String>,
    },
    Done(Finished),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashStep {
    Ready,
    Waiting,
    Done,
}

#[derive(Debug, Clone)]
pub(super) struct Flash {
    build: Finished,
    step: FlashStep,
    backup: Option<PathBuf>,
    /// Layers as backed up before flashing, to compare after.
    pages: Option<filco_core::LayerSet>,
    saw_unplug: bool,
    firmware_ok: Option<bool>,
    firmware_name: Option<String>,
    layers_changed: Vec<usize>,
}

/// Firmware state of the app.
pub(super) struct Fw {
    pub(super) stock: Result<Arc<Stock>, String>,
    pub(super) state: PathBuf,
    pub(super) draft: Option<FwDraft>,
    /// Checksum tag of the firmware on the keyboard, when its info block rebuilds to a known image.
    on_keyboard_tag: Option<u16>,
    pub(super) build: Option<BuildState>,
    generation: u64,
    pub(super) flash: Option<Flash>,
    pub(super) launcher: Launcher,
}

impl Fw {
    pub(super) fn new(stock_dir: Option<&Path>, state: &Path, launcher: Launcher) -> Self {
        let stock = stock_dir
            .ok_or_else(|| "no firmware folder".to_owned())
            .and_then(|d| Stock::load(d).map_err(|e| e.to_string()))
            .map(Arc::new);
        Fw {
            stock,
            state: state.to_owned(),
            draft: None,
            on_keyboard_tag: None,
            build: None,
            generation: 0,
            flash: None,
            launcher,
        }
    }

    pub(super) fn builds_root(&self) -> PathBuf {
        self.state.join("firmware")
    }
}

/// What a build needs, taken out of the app so the 3-4 s pipeline runs without holding the app lock.
#[derive(Debug, Clone)]
pub struct BuildJob {
    stock: Arc<Stock>,
    config: Config,
    root: PathBuf,
    generation: u64,
}

impl BuildJob {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Runs the pipeline; `line` gets every report line as it is written.
    pub fn run(&self, line: &mut dyn FnMut(&str)) -> Finished {
        match fw::run_build(&self.stock, &self.config, &self.root, line) {
            Ok(o) => Finished {
                config: self.config.clone(),
                tag: Some(o.tag),
                dir: Some(o.dir),
                passed: o.passed,
                has_updater: o.updater.is_some(),
                report: o.report,
                error: None,
            },
            Err(e) => Finished {
                config: self.config.clone(),
                tag: None,
                dir: None,
                passed: false,
                has_updater: false,
                report: Vec::new(),
                error: Some(e.to_string()),
            },
        }
    }
}

fn is_valid_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Same firmware apart from the name.
fn same_content(a: &Config, b: &Config) -> bool {
    let mut b = b.clone();
    b.name.clone_from(&a.name);
    *a == b
}

fn tap_hold_view(t: TapHold) -> FwTapHoldView {
    FwTapHoldView {
        tap: t.tap,
        hold: match t.hold {
            Hold::Key(code) => FwHoldView::Key { code },
            Hold::Layer(layer) => FwHoldView::Layer { layer },
        },
        term_ms: t.term_ms,
    }
}

fn tap_hold_of(v: &FwTapHoldView) -> TapHold {
    TapHold {
        tap: v.tap,
        hold: match v.hold {
            FwHoldView::Key { code } => Hold::Key(code),
            FwHoldView::Layer { layer } => Hold::Layer(layer),
        },
        term_ms: v.term_ms,
    }
}

fn key_view(k: &FwKey, lang: UiLang) -> FwKeyView {
    let code = |c: u8| FwCodeView::new(c, lang);
    let (tap_label, hold_label) = match k.tap_hold {
        Some(t) => (
            Some(code(t.tap).legend),
            Some(match t.hold {
                Hold::Key(c) => code(c).legend,
                Hold::Layer(l) => format!("\u{2192} L{}", l + 1),
            }),
        ),
        None => (None, None),
    };
    FwKeyView {
        index: k.index,
        fn_code: k.fn_code.map(code),
        fn_base: k.fn_base.map(code),
        fn_stock: k.fn_stock.map(code),
        default: code(k.default),
        default_base: code(k.default_base),
        default_stock: code(k.default_stock),
        tap_hold: k.tap_hold.map(tap_hold_view),
        tap_label,
        hold_label,
        changed: k.changed(),
        custom: k.custom(),
        fn_locked: k.fn_locked.map(str::to_owned),
        tap_hold_locked: k.tap_hold_locked.map(str::to_owned),
    }
}

impl App {
    /// Where the stock HEX and updater are (known only once Tauri resolved its resource folder).
    pub fn set_firmware_dir(&mut self, dir: &Path) {
        self.fw.stock = Stock::load(dir).map(Arc::new).map_err(|e| e.to_string());
    }

    fn stock(&self) -> Result<Arc<Stock>, ErrorView> {
        self.fw
            .stock
            .clone()
            .map_err(|e| ErrorView::new("fw_unavailable", e))
    }

    /// After every successful read: the firmware on the keyboard is the draft's base. The draft is created on
    /// the first read (from the autosaved draft when there is one) and rebased when the firmware changed.
    pub(super) fn fw_after_read(&mut self) {
        let Ok(stock) = self.fw.stock.clone() else { return };
        let info = self.conn.as_ref().and_then(|c| c.info.clone());
        let (base, tag) = match info.as_ref().map(config_from_info) {
            Some(cfg) => match stock.build(&cfg) {
                Ok(b) => (cfg, Some(b.tag())),
                // A block this app cannot rebuild (a newer format): start from stock.
                Err(_) => (stock_config(&cfg.name), None),
            },
            None => (stock_config("custom"), None),
        };
        self.fw.on_keyboard_tag = tag;
        match self.fw.draft.as_mut() {
            // Another firmware now: a draft without edits follows it, one with edits keeps them.
            Some(d) if *d.base() != base && !d.is_changed() => {
                self.fw.draft = FwDraft::new(&stock, base).ok();
                if let Some(d) = self.fw.draft.as_ref() {
                    let _ = fw::save(&self.fw.state, &stock, d);
                }
            }
            Some(d) if *d.base() != base => {
                if d.rebase(&stock, base).is_err() {
                    self.fw.draft = None;
                }
            }
            Some(_) => {}
            None => {
                if let Ok(mut d) = FwDraft::new(&stock, base) {
                    if let Some(saved) = fw::load_saved(&self.fw.state, &stock) {
                        let _ = d.replace(&stock, saved);
                    }
                    self.fw.draft = Some(d);
                }
            }
        }
        self.fw_check_flash();
    }

    fn fw_edit_with(&mut self, f: impl FnOnce(&Stock, &mut FwDraft) -> Result<(), filco_firmware::Error>) {
        self.error = None;
        let stock = match self.stock() {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let Some(d) = self.fw.draft.as_mut() else {
            self.error = Some(ErrorView::new("not_read", "keyboard not read"));
            return;
        };
        if let Err(e) = f(&stock, d) {
            self.error = Some(ErrorView::new("fw_edit_refused", e));
        }
        if let Err(e) = fw::save(&self.fw.state, &stock, d) {
            self.error = Some(ErrorView::new("fw_save_failed", e));
        }
    }

    pub fn fw_edit(&mut self, edit: &FwEditInput) {
        let e = match *edit {
            FwEditInput::SetFn { key, code } => FwEdit::SetFn { key, code },
            FwEditInput::SetDefault { key, code } => FwEdit::SetDefault { key, code },
            FwEditInput::SetTapHold { key, ref tap_hold } => FwEdit::SetTapHold {
                key,
                tap_hold: tap_hold.as_ref().map(tap_hold_of),
            },
            FwEditInput::KeyToBase { key } => FwEdit::KeyToBase { key },
            FwEditInput::KeyToStock { key } => FwEdit::KeyToStock { key },
            FwEditInput::SetLayerTapHold { on } => FwEdit::SetLayerTapHold(on),
        };
        self.fw_edit_with(|s, d| d.apply(s, &e));
    }

    pub fn fw_undo(&mut self) {
        self.fw_edit_with(|s, d| {
            d.undo(s);
            Ok(())
        });
    }

    pub fn fw_redo(&mut self) {
        self.fw_edit_with(|s, d| {
            d.redo(s);
            Ok(())
        });
    }

    pub fn fw_revert(&mut self) {
        self.fw_edit_with(|s, d| {
            d.revert(s);
            Ok(())
        });
    }

    /// Imports a TOML configuration into the draft (one undoable step).
    pub fn fw_import(&mut self, path: &Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.error = Some(ErrorView::new("import_failed", e));
                return;
            }
        };
        self.fw_edit_with(|s, d| {
            let cfg = s.parse(&text)?;
            d.replace(s, cfg)
        });
    }

    pub fn fw_export(&mut self, path: &Path) {
        let (Ok(stock), Some(d)) = (self.stock(), self.fw.draft.as_ref()) else {
            return;
        };
        self.error = std::fs::write(path, stock.to_toml(d.working()))
            .err()
            .map(|e| ErrorView::new("export_failed", e));
    }

    /// Takes what a build needs. `None` (with the error set) when it cannot start.
    pub fn fw_start_build(&mut self, name: &str) -> Option<BuildJob> {
        self.error = None;
        if !is_valid_name(name) {
            self.error = Some(ErrorView::new("fw_bad_name", name));
            return None;
        }
        let stock = match self.stock() {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(e);
                return None;
            }
        };
        let d = self.fw.draft.as_ref()?;
        let mut config = d.working().clone();
        name.clone_into(&mut config.name);
        self.fw.generation += 1;
        self.fw.build = Some(BuildState::Running {
            name: name.to_owned(),
            generation: self.fw.generation,
            lines: Vec::new(),
        });
        Some(BuildJob {
            stock,
            config,
            root: self.fw.builds_root(),
            generation: self.fw.generation,
        })
    }

    /// A report line of the running build (ignored when that build was cancelled).
    pub fn fw_build_line(&mut self, generation: u64, line: &str) {
        if let Some(BuildState::Running {
            generation: g, lines, ..
        }) = self.fw.build.as_mut()
            && *g == generation
        {
            lines.push(line.to_owned());
        }
    }

    pub fn fw_finish_build(&mut self, job: &BuildJob, done: Finished) {
        if matches!(self.fw.build, Some(BuildState::Running { generation, .. }) if generation == job.generation)
        {
            self.fw.build = Some(BuildState::Done(done));
        }
    }

    pub fn fw_close_build(&mut self) {
        self.fw.generation += 1;
        self.fw.build = None;
    }

    /// Steps 1-2 of flashing: back up the four layers and check FILCO Assist is closed. Only for the build that
    /// just passed, while the draft still equals it.
    pub fn fw_flash_prepare(&mut self) {
        self.error = None;
        let Some(BuildState::Done(b)) = self.fw.build.clone() else {
            return;
        };
        let fresh = self
            .fw
            .draft
            .as_ref()
            .is_some_and(|d| same_content(d.working(), &b.config));
        if !b.passed || !b.has_updater || !fresh {
            self.error = Some(ErrorView::new(
                "fw_not_flashable",
                "build is not the current passed draft",
            ));
            return;
        }
        if let Err(e) = ensure_assist_closed(self.backend.as_ref()) {
            self.error = Some(guard_error(&e));
            return;
        }
        let Some(conn) = self.conn.as_mut() else { return };
        let Some(model) = self.models.get(conn.model) else {
            return;
        };
        let pages = match conn.device.read_all(&mut |_| {}) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(super::read_error(&e));
                return;
            }
        };
        let backup = match self.store.save(
            model,
            conn.device.info(),
            &pages,
            "before_flash",
            &timestamp_now(),
        ) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(ErrorView::new("backup_failed", e));
                return;
            }
        };
        self.fw.flash = Some(Flash {
            build: b,
            step: FlashStep::Ready,
            backup: Some(backup),
            pages: Some(pages),
            saw_unplug: false,
            firmware_ok: None,
            firmware_name: None,
            layers_changed: Vec::new(),
        });
    }

    /// Step 3: start the updater (the user pressed the button). The user checks the CheckSum and presses Start.
    pub fn fw_flash_launch(&mut self) {
        self.error = None;
        let Some(f) = self.fw.flash.as_mut() else { return };
        let Some(dir) = f.build.dir.clone() else { return };
        if let Err(e) = ensure_assist_closed(self.backend.as_ref()) {
            self.error = Some(guard_error(&e));
            return;
        }
        match (self.fw.launcher)(&dir) {
            Ok(()) => f.step = FlashStep::Waiting,
            Err(e) => self.error = Some(ErrorView::new("fw_launch_failed", e)),
        }
    }

    pub fn fw_flash_close(&mut self) {
        self.fw.flash = None;
    }

    /// While waiting: note the keyboard going away (it is in the updater's bootloader) and, once it is back and
    /// read, check what it runs.
    pub(super) fn fw_check_flash(&mut self) {
        let connected = self.conn.as_ref().is_some_and(|c| c.editor.is_some());
        let Some(f) = self.fw.flash.as_mut() else { return };
        if f.step != FlashStep::Waiting {
            return;
        }
        if self.conn.is_none() {
            f.saw_unplug = true;
            return;
        }
        if !f.saw_unplug || !connected {
            return;
        }
        let Some(conn) = self.conn.as_ref() else { return };
        f.firmware_name = conn.info.as_ref().map(|i| i.name.clone());
        f.firmware_ok = Some(
            f.firmware_name.as_deref() == Some(f.build.config.name.as_str())
                && self.fw.on_keyboard_tag.is_some()
                && self.fw.on_keyboard_tag == f.build.tag,
        );
        if let (Some(before), Some(ed)) = (f.pages.as_ref(), conn.editor.as_ref()) {
            f.layers_changed = before.changed_layers(ed.base());
        }
        f.step = FlashStep::Done;
        if f.firmware_ok == Some(true) {
            // The draft was this build (only its name differed): start afresh from what the keyboard now runs.
            if let (Ok(stock), Some(d)) = (self.fw.stock.clone(), self.fw.draft.as_ref())
                && same_content(d.working(), d.base())
            {
                self.fw.draft = FwDraft::new(&stock, d.base().clone()).ok();
                if let Some(d) = self.fw.draft.as_ref() {
                    let _ = fw::save(&self.fw.state, &stock, d);
                }
            }
        }
    }

    pub fn fw_builds(&self) -> Vec<FwBuiltView> {
        let on_keyboard = self
            .conn
            .as_ref()
            .and_then(|c| c.info.as_ref())
            .map(|i| i.name.clone());
        fw::list_builds(&self.fw.builds_root())
            .into_iter()
            .map(|b: Built| FwBuiltView {
                on_keyboard: on_keyboard.as_deref() == Some(b.name.as_str())
                    && self.fw.on_keyboard_tag.is_some()
                    && self.fw.on_keyboard_tag == b.tag,
                passed: b.updater.is_some(),
                dir: b.dir.to_string_lossy().into_owned(),
                name: b.name,
                tag: b.tag,
                modified: b.modified,
            })
            .collect()
    }

    pub fn fw_dir(&self) -> PathBuf {
        self.fw.builds_root()
    }

    /// A name for the next build: the keyboard's build name with the next free `_<n>`.
    fn suggested_name(&self, d: &FwDraft) -> String {
        let base = d.base().name.as_str();
        let stem = base
            .rsplit_once('_')
            .filter(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .map_or(base, |(s, _)| s);
        let taken: Vec<String> = fw::list_builds(&self.fw.builds_root())
            .into_iter()
            .map(|b| b.name)
            .collect();
        (2..1000)
            .map(|n| format!("{stem}_{n}"))
            .find(|n| n.len() <= 32 && !taken.contains(n) && n != base)
            .unwrap_or_else(|| "custom".to_owned())
    }

    pub(super) fn fw_view(&self) -> Option<FwView> {
        let model = self.model()?;
        let conn = self.conn.as_ref()?;
        conn.editor.as_ref()?;
        let on_keyboard = conn.info.as_ref().map(|i| i.name.clone());
        let stock_error = self.fw.stock.as_ref().err().cloned();
        let Some(d) = self.fw.draft.as_ref() else {
            return Some(FwView {
                stock_error: stock_error.or_else(|| Some("draft unavailable".to_owned())),
                on_keyboard,
                on_keyboard_tag: self.fw.on_keyboard_tag,
                keys: Vec::new(),
                changes: 0,
                can_undo: false,
                can_redo: false,
                fn_slots_used: 0,
                fn_slots: 39,
                tap_key_used: 0,
                tap_layer_used: 0,
                layer_tap_hold: false,
                fn_key: 0,
                suggested_name: String::new(),
                build: None,
                flash: None,
            });
        };
        let fn_key = d.fn_position();
        let w = d.working();
        let (layer_tap_hold, layer_tap_hold_base) = d.layer_tap_hold();
        let build = self.fw.build.as_ref().map(|b| match b {
            BuildState::Running { name, lines, .. } => FwBuildView {
                name: name.clone(),
                running: true,
                steps: steps(lines),
                passed: None,
                tag: None,
                dir: None,
                has_updater: false,
                failures: Vec::new(),
                report: Vec::new(),
                error: None,
                stale: false,
            },
            BuildState::Done(f) => FwBuildView {
                name: f.config.name.clone(),
                running: false,
                steps: steps(&f.report),
                passed: Some(f.passed),
                tag: f.tag,
                dir: f.dir.as_ref().map(|p| p.to_string_lossy().into_owned()),
                has_updater: f.has_updater,
                failures: f
                    .report
                    .iter()
                    .filter(|l| l.contains("FAIL") || l.contains("emulator stopped"))
                    .cloned()
                    .collect(),
                report: f.report.clone(),
                error: f.error.clone(),
                stale: !same_content(w, &f.config),
            },
        });
        let flash = self.fw.flash.as_ref().map(|f| FwFlashView {
            name: f.build.config.name.clone(),
            tag: f.build.tag.unwrap_or(0),
            step: match f.step {
                FlashStep::Ready => "ready",
                FlashStep::Waiting => "waiting",
                FlashStep::Done => "done",
            }
            .to_owned(),
            backup: f.backup.as_ref().map(|p| p.to_string_lossy().into_owned()),
            saw_unplug: f.saw_unplug,
            firmware_ok: f.firmware_ok,
            firmware_name: f.firmware_name.clone(),
            layers_changed: f.layers_changed.clone(),
        });
        Some(FwView {
            stock_error,
            on_keyboard,
            on_keyboard_tag: self.fw.on_keyboard_tag,
            keys: model
                .layout
                .keys
                .iter()
                .map(|k| key_view(&d.key(k.index, fn_key), self.lang))
                .collect(),
            changes: u32::try_from(d.changed_keys().len()).unwrap_or(0)
                + u32::from(layer_tap_hold != layer_tap_hold_base),
            can_undo: d.can_undo(),
            can_redo: d.can_redo(),
            fn_slots_used: u32::try_from(d.fn_slots_used()).unwrap_or(0),
            fn_slots: u32::from(filco_firmware::v72::FN_ENTRIES),
            tap_key_used: u32::from(w.tap_key.is_some()),
            tap_layer_used: u32::try_from(w.tap_layers.len()).unwrap_or(0),
            layer_tap_hold,
            fn_key,
            suggested_name: self.suggested_name(d),
            build,
            flash,
        })
    }
}

fn steps(lines: &[String]) -> Vec<FwStepView> {
    fw::report_steps(lines)
        .into_iter()
        .map(|s| FwStepView {
            id: s.id.to_owned(),
            ok: s.ok,
            fail: s.fail,
        })
        .collect()
}
