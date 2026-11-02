//! Application state behind the Tauri commands. No Tauri types here, so it is tested like any other code.

mod firmware;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use filco_core::model::{KnownDevices, Support, builtin};
use filco_core::slot::type_code;
use filco_core::{LayerSet, ModelDescriptor, Slot, SlotChange, export};
use filco_protocol::{Phase as IoPhase, Progress, ProtocolError, TransportError};
use filco_service::assist_config;
use filco_service::backend::Backend;
use filco_service::device::Device;
use filco_service::discovery::{FoundKeyboard, find_keyboards};
use filco_service::editor::Editor;
use filco_service::guard::{GuardError, ensure_assist_closed};
use filco_service::journal::{Journal, JournalFile, LayerState};
use filco_service::store::{self, BackupStore, timestamp_now};
use filco_service::writer::{self, LayerOutcome, PrepareError, WriteContext, WritePlan, WriteReport};

use crate::view::{
    Action, AssistView, BackupView, ChangeView, DeviceView, ErrorView, FirmwareView, JournalLayerView,
    JournalView, LinkedLayerView, ModelView, OutcomeView, Phase, PlanView, ProgressView, ReportView,
    SlotView, Snapshot, UiLang,
};

pub use firmware::{Launcher, launch_updater};

/// Where the app keeps its files.
#[derive(Debug, Clone)]
pub struct Paths {
    pub backups: PathBuf,
    /// Folder of `journal.json`.
    pub state: PathBuf,
    /// FILCO Assist's settings file (read only), if it can exist on this machine.
    pub assist_config: Option<PathBuf>,
    /// Folder of the stock V72 HEX and updater shipped with the app (firmware mode).
    pub firmware: Option<PathBuf>,
}

impl Paths {
    pub fn default_for_user() -> Option<Self> {
        let backups = BackupStore::default_root()?;
        let state = backups.parent()?.to_owned();
        Some(Paths {
            backups,
            state,
            assist_config: assist_config::default_path(),
            firmware: None,
        })
    }
}

struct Conn {
    device: Device,
    model: usize,
    editor: Option<Editor>,
    /// The firmware info block (command 0x30), read once per successful layer read; `None` when the firmware
    /// has no block.
    info: Option<filco_fwinfo::Info>,
}

pub struct App {
    models: Vec<ModelDescriptor>,
    known: KnownDevices,
    backend: Arc<dyn Backend>,
    store: BackupStore,
    journal: JournalFile,
    assist_config: Option<PathBuf>,
    lang: UiLang,
    found: Vec<FoundKeyboard>,
    conn: Option<Conn>,
    read_failed: bool,
    plan: Option<WritePlan>,
    report: Option<WriteReport>,
    error: Option<ErrorView>,
    simulated: bool,
    fw: firmware::Fw,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("found", &self.found.len())
            .finish_non_exhaustive()
    }
}

/// Some key of the model has a tap-hold slot (type 8) in some layer.
fn has_tap_hold_slot(m: &ModelDescriptor, layers: &LayerSet) -> bool {
    let a = m.addressing();
    (0..layers.len()).any(|l| {
        m.layout.keys.iter().any(|k| {
            layers
                .slot(a, l, k.index)
                .is_some_and(|s| s.kind().type_code() == type_code::TAP_HOLD)
        })
    })
}

fn read_error(e: &ProtocolError) -> ErrorView {
    let code = match e {
        ProtocolError::Transport(TransportError::Disconnected) => "disconnected",
        _ => "read_failed",
    };
    ErrorView::new(code, e)
}

fn guard_error(e: &GuardError) -> ErrorView {
    match e {
        GuardError::AssistRunning(_) => ErrorView::new("assist_running", e),
        GuardError::Probe(_) => ErrorView::new("probe_failed", e),
    }
}

/// Counts page sessions so the UI can show one progress bar for a whole operation.
fn progress_counter(steps: u32, sink: &mut dyn FnMut(ProgressView)) -> impl FnMut(Progress) + '_ {
    let mut step = 0u32;
    move |p: Progress| {
        if p.done == p.total {
            step += 1;
        }
        sink(ProgressView {
            layer: p.layer,
            phase: match p.phase {
                IoPhase::Read => "read",
                IoPhase::Write => "write",
                IoPhase::Verify => "verify",
                IoPhase::Info => "info",
            }
            .to_owned(),
            done: p.done,
            total: p.total,
            step: step.min(steps),
            steps,
        });
    }
}

impl App {
    pub fn new(backend: Arc<dyn Backend>, paths: &Paths, simulated: bool) -> Self {
        App {
            models: builtin::models().unwrap_or_default(),
            known: builtin::known_devices().unwrap_or(KnownDevices {
                schema: None,
                devices: Vec::new(),
            }),
            backend,
            store: BackupStore::new(&paths.backups),
            journal: JournalFile::new(&paths.state),
            assist_config: paths.assist_config.clone(),
            lang: UiLang::Vi,
            found: Vec::new(),
            conn: None,
            read_failed: false,
            plan: None,
            report: None,
            error: None,
            simulated,
            fw: firmware::Fw::new(paths.firmware.as_deref(), &paths.state, Box::new(launch_updater)),
        }
    }

    /// How the updater is started (tests and the E2E build simulate a flash).
    pub fn set_launcher(&mut self, launcher: Launcher) {
        self.fw.launcher = launcher;
    }

    pub fn set_lang(&mut self, lang: UiLang) {
        self.lang = lang;
    }

    fn model(&self) -> Option<&ModelDescriptor> {
        self.conn.as_ref().and_then(|c| self.models.get(c.model))
    }

    pub fn model_view(&self, id: &str) -> Option<ModelView> {
        self.models
            .iter()
            .find(|m| m.id == id)
            .map(|m| ModelView::new(m, self.lang))
    }

    fn device_view(&self, f: &FoundKeyboard) -> DeviceView {
        let model = f.model.and_then(|i| self.models.get(i));
        DeviceView {
            path: f.info.path.clone(),
            name: model
                .map(|m| m.name.clone())
                .or_else(|| f.known_name.clone())
                .unwrap_or_else(|| format!("{:04X}:{:04X}", f.info.vendor_id, f.info.product_id)),
            vendor_id: f.info.vendor_id,
            product_id: f.info.product_id,
            version: f.info.version,
            model_id: model.map(|m| m.id.clone()),
        }
    }

    /// Looks for keyboards; connects to the first supported one if none is connected and reads it when FILCO
    /// Assist is closed. Drops the connection when its keyboard is gone.
    pub fn refresh(&mut self, progress: &mut dyn FnMut(ProgressView)) {
        self.rescan();
        self.fw_check_flash();
        if self.conn.is_none()
            && let Some(f) = self.found.iter().find(|f| f.model.is_some()).cloned()
        {
            self.connect_found(&f, progress);
        }
    }

    /// Lists keyboards again and drops the connection if its keyboard is gone. Never connects.
    fn rescan(&mut self) {
        let interfaces = match self.backend.interfaces() {
            Ok(i) => i,
            Err(e) => {
                self.error = Some(ErrorView::new("enumerate_failed", e));
                Vec::new()
            }
        };
        self.found = find_keyboards(&interfaces, &self.models, &self.known);
        if let Some(c) = &self.conn
            && !self.found.iter().any(|f| f.info.path == c.device.info().path)
        {
            self.conn = None;
            self.plan = None;
        }
    }

    /// The keyboard stopped answering: forget the connection and look again.
    fn lost(&mut self) {
        self.conn = None;
        self.plan = None;
        self.rescan();
    }

    fn connect_found(&mut self, f: &FoundKeyboard, progress: &mut dyn FnMut(ProgressView)) {
        let Some(mi) = f.model else { return };
        let Some(model) = self.models.get(mi) else { return };
        match Device::open(self.backend.as_ref(), &f.info, model) {
            Ok(device) => {
                self.conn = Some(Conn {
                    device,
                    model: mi,
                    editor: None,
                    info: None,
                });
                self.read_failed = false;
                self.report = None;
                if ensure_assist_closed(self.backend.as_ref()).is_ok() {
                    self.read(progress);
                }
            }
            Err(e) => self.error = Some(ErrorView::new("open_failed", e)),
        }
    }

    /// Connects to the keyboard at `path` (from the device list).
    pub fn connect(&mut self, path: &str, progress: &mut dyn FnMut(ProgressView)) {
        let Some(f) = self.found.iter().find(|f| f.info.path == path).cloned() else {
            self.error = Some(ErrorView::new("disconnected", "keyboard not found"));
            return;
        };
        self.conn = None;
        self.plan = None;
        self.connect_found(&f, progress);
    }

    /// Reads every layer. Discards edits: the UI asks first when there are any.
    pub fn read(&mut self, progress: &mut dyn FnMut(ProgressView)) {
        self.error = None;
        if let Err(e) = ensure_assist_closed(self.backend.as_ref()) {
            self.error = Some(guard_error(&e));
            return;
        }
        let Some(conn) = self.conn.as_mut() else { return };
        let Some(model) = self.models.get(conn.model) else {
            return;
        };
        let steps = u32::try_from(model.layers.len()).unwrap_or(0);
        let mut counter = progress_counter(steps, progress);
        match conn.device.read_all(&mut counter) {
            Ok(pages) => {
                self.read_failed = false;
                // Read the firmware info block once, after the layers. A firmware without one (NACK) leaves
                // info None; a read error here is not fatal to editing, so it is ignored (info stays None).
                conn.info = conn.device.read_info(&mut |_| {}).ok().flatten();
                match Editor::new(model, pages) {
                    Ok(ed) => conn.editor = Some(ed),
                    Err(e) => self.error = Some(ErrorView::new("read_failed", e)),
                }
                self.fw_after_read();
            }
            Err(e) => {
                self.read_failed = true;
                self.error = Some(read_error(&e));
                if matches!(e, ProtocolError::Transport(TransportError::Disconnected)) {
                    self.lost();
                }
            }
        }
        self.plan = None;
    }

    fn editing(&mut self) -> Result<(&ModelDescriptor, &mut Editor), ErrorView> {
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| ErrorView::new("disconnected", "no keyboard"))?;
        let model = self
            .models
            .get(conn.model)
            .ok_or_else(|| ErrorView::new("disconnected", "no model"))?;
        let ed = conn
            .editor
            .as_mut()
            .ok_or_else(|| ErrorView::new("not_read", "layers not read"))?;
        Ok((model, ed))
    }

    fn edit(&mut self, f: impl FnOnce(&ModelDescriptor, &mut Editor) -> Result<(), ErrorView>) {
        self.plan = None;
        self.report = None;
        self.error = match self.editing() {
            Ok((m, ed)) => f(m, ed).err(),
            Err(e) => Some(e),
        };
    }

    /// Gives key `index` the action in `layers`. A tap-hold slot is accepted whatever the keyboard runs: saving
    /// then updates the firmware (see [`Snapshot::tap_hold_upgrade`]). When the keyboard still runs tap-hold set in
    /// the firmware, the first edit of a tap-hold (or of one of those keys) also turns those into slots of every
    /// layer, in the same undo step, so nothing they did is lost when the new firmware takes over.
    pub fn assign(&mut self, layers: &[usize], index: u8, action: &Action) {
        let older = self.firmware_tap_slots();
        self.edit(|m, ed| {
            let slot = action.to_slot().map_err(|e| ErrorView::new("edit_invalid", e))?;
            let mut edits: Vec<(usize, u8, Slot)> = layers.iter().map(|&l| (l, index, slot)).collect();
            let tap_hold = slot.kind().type_code() == type_code::TAP_HOLD;
            if !older.is_empty()
                && !has_tap_hold_slot(m, ed.working())
                && (tap_hold || older.iter().any(|&(k, _)| k == index))
            {
                for &(k, s) in &older {
                    for l in 0..ed.working().len() {
                        if !(k == index && layers.contains(&l)) {
                            edits.push((l, k, s));
                        }
                    }
                }
            }
            ed.assign_many(m, &edits)
                .map(|_| ())
                .map_err(|e| ErrorView::new("edit_invalid", e))
        });
    }

    /// The tap-hold keys the firmware on the keyboard holds itself (an older build), as layer slots of type 8;
    /// empty when the firmware reads tap-hold from the slots already, or has none.
    fn firmware_tap_slots(&self) -> Vec<(u8, Slot)> {
        let Some(info) = self.conn.as_ref().and_then(|c| c.info.as_ref()) else {
            return Vec::new();
        };
        if info.layer_tap_hold.is_some() {
            return Vec::new();
        }
        let term = |ms: u16| u8::try_from(ms.div_ceil(10)).unwrap_or(u8::MAX).max(1);
        let key = info
            .tap_key
            .iter()
            .map(|t| (t.key, Slot([type_code::TAP_HOLD, t.tap, t.hold, term(t.term_ms)])));
        let layer = info.tap_layers.iter().map(|t| {
            let hold = filco_core::slot::HOLD_LAYER + t.layer;
            (t.key, Slot([type_code::TAP_HOLD, t.tap, hold, term(t.term_ms)]))
        });
        key.chain(layer).collect()
    }

    pub fn copy_layer(&mut self, from: usize, to: usize) {
        self.edit(|m, ed| {
            ed.copy_layer(m, from, to)
                .map(|_| ())
                .map_err(|e| ErrorView::new("edit_invalid", e))
        });
    }

    pub fn undo(&mut self) {
        self.edit(|m, ed| {
            ed.undo(m);
            Ok(())
        });
    }

    pub fn redo(&mut self) {
        self.edit(|m, ed| {
            ed.redo(m);
            Ok(())
        });
    }

    pub fn revert(&mut self) {
        self.edit(|_, ed| {
            ed.revert();
            Ok(())
        });
    }

    /// Puts `pages` into the editor as one undoable step; writing them still goes through the diff.
    fn load_pages(&mut self, pages: &LayerSet) {
        self.edit(|m, ed| {
            ed.replace_all(m, pages)
                .map(|_| ())
                .map_err(|e| ErrorView::new("import_failed", e))
        });
    }

    pub fn load_backup(&mut self, dir: &Path) {
        let Some(model) = self.model().cloned() else {
            return;
        };
        match store::load(dir, &model) {
            Ok((_, pages)) => self.load_pages(&pages),
            Err(e) => self.error = Some(ErrorView::new("backup_invalid", e)),
        }
    }

    pub fn import_file(&mut self, path: &Path) {
        let Some(model) = self.model().cloned() else {
            return;
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.error = Some(ErrorView::new("import_failed", e));
                return;
            }
        };
        match export::parse(&text, &model) {
            Ok(pages) => self.load_pages(&pages),
            Err(e) => self.error = Some(ErrorView::new("import_failed", e)),
        }
    }

    /// Writes the edited pages (what the user sees) to an export file.
    pub fn export_file(&mut self, path: &Path) {
        let Some(conn) = self.conn.as_ref() else { return };
        let (Some(model), Some(ed)) = (self.models.get(conn.model), conn.editor.as_ref()) else {
            return;
        };
        let file = export::build(model, ed.working(), env!("CARGO_PKG_VERSION"), &timestamp_now());
        self.error = std::fs::write(path, export::to_json(&file))
            .err()
            .map(|e| ErrorView::new("export_failed", e));
    }

    /// Reads the keyboard fresh and saves a backup.
    pub fn backup_now(&mut self, progress: &mut dyn FnMut(ProgressView)) -> Option<PathBuf> {
        self.error = None;
        if let Err(e) = ensure_assist_closed(self.backend.as_ref()) {
            self.error = Some(guard_error(&e));
            return None;
        }
        let conn = self.conn.as_mut()?;
        let model = self.models.get(conn.model)?;
        let mut counter = progress_counter(u32::try_from(model.layers.len()).unwrap_or(0), progress);
        let pages = match conn.device.read_all(&mut counter) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(read_error(&e));
                return None;
            }
        };
        match self
            .store
            .save(model, conn.device.info(), &pages, "manual", &timestamp_now())
        {
            Ok(p) => Some(p),
            Err(e) => {
                self.error = Some(ErrorView::new("backup_failed", e));
                None
            }
        }
    }

    pub fn backups(&self) -> Vec<BackupView> {
        let Some(model) = self.model() else {
            return Vec::new();
        };
        self.store
            .list(model)
            .into_iter()
            .map(|b| BackupView {
                name: b.name,
                path: b.path.to_string_lossy().into_owned(),
                created: b.created,
                reason: b.reason,
                problem: b.problem,
            })
            .collect()
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.model()
            .map_or_else(|| self.store.root().to_owned(), |m| self.store.model_dir(m))
    }

    /// Steps 1-4 of the safe write: checks, fresh read, compare, backup. Shows the diff dialog on success.
    pub fn prepare_write(&mut self, progress: &mut dyn FnMut(ProgressView)) {
        self.error = None;
        self.report = None;
        let Some(conn) = self.conn.as_mut() else { return };
        let (Some(model), Some(ed)) = (self.models.get(conn.model), conn.editor.as_mut()) else {
            return;
        };
        let ctx = WriteContext {
            store: &self.store,
            journal: &self.journal,
            probe: self.backend.as_ref(),
        };
        let mut counter = progress_counter(u32::try_from(model.layers.len()).unwrap_or(0), progress);
        match writer::prepare(&mut conn.device, model, ed, &ctx, &timestamp_now(), &mut counter) {
            Ok(plan) => self.plan = Some(plan),
            Err(e) => {
                self.error = Some(match &e {
                    PrepareError::Guard(g) => guard_error(g),
                    PrepareError::ReadOnlyModel => ErrorView::new("read_only_model", &e),
                    PrepareError::JournalPending(_) | PrepareError::Journal(_) => {
                        ErrorView::new("journal_pending", &e)
                    }
                    PrepareError::NothingToWrite => ErrorView::new("nothing_to_write", &e),
                    PrepareError::Read(r) => read_error(r),
                    PrepareError::DeviceChanged { .. } => ErrorView::new("device_changed", &e),
                    PrepareError::Backup(_) => ErrorView::new("backup_failed", &e),
                });
                if let PrepareError::DeviceChanged { pages, .. } = e {
                    // Keep the edits, now measured against what the keyboard really holds.
                    ed.rebase(pages);
                }
            }
        }
    }

    pub fn cancel_write(&mut self) {
        self.plan = None;
    }

    /// Steps 6-9, after the user confirmed the diff.
    pub fn execute_write(&mut self, progress: &mut dyn FnMut(ProgressView)) {
        self.error = None;
        let Some(plan) = self.plan.take() else { return };
        let Some(conn) = self.conn.as_mut() else { return };
        let ctx = WriteContext {
            store: &self.store,
            journal: &self.journal,
            probe: self.backend.as_ref(),
        };
        let steps = u32::try_from(plan.layers.len() * 2).unwrap_or(0);
        let mut counter = progress_counter(steps, progress);
        match writer::execute(&mut conn.device, &plan, &ctx, &timestamp_now(), &mut counter) {
            Ok(report) => {
                let ok = report.succeeded();
                self.report = Some(report);
                if ok {
                    if let Some(ed) = conn.editor.as_mut() {
                        ed.rebase(plan.target.clone());
                    }
                } else {
                    self.after_failed_write();
                }
            }
            Err(e) => {
                self.error = Some(match &e {
                    writer::ExecuteError::Guard(g) => guard_error(g),
                    writer::ExecuteError::Journal(_) => ErrorView::new("journal_pending", &e),
                    writer::ExecuteError::WrongDevice => ErrorView::new("disconnected", &e),
                });
            }
        }
    }

    /// After a failed write the keyboard may hold anything: read it again so the editor compares with reality.
    fn after_failed_write(&mut self) {
        let Some(conn) = self.conn.as_mut() else { return };
        match conn.device.read_all(&mut |_| {}) {
            Ok(pages) => {
                if let Some(ed) = conn.editor.as_mut() {
                    ed.rebase(pages);
                }
            }
            Err(_) => self.lost(),
        }
    }

    pub fn dismiss_report(&mut self) {
        self.report = None;
    }

    /// The user keeps the keyboard as it is after an unfinished write.
    pub fn dismiss_journal(&mut self) {
        self.error = self
            .journal
            .finish()
            .err()
            .map(|e| ErrorView::new("journal_pending", e));
    }

    /// Loads the backup named by the unfinished write's journal into the editor; the user then writes it with the
    /// usual diff and confirmation.
    pub fn restore_from_journal(&mut self) {
        let journal = match self.journal.pending() {
            Ok(Some(j)) => j,
            Ok(None) => return,
            Err(e) => {
                self.error = Some(ErrorView::new("journal_pending", e));
                return;
            }
        };
        self.load_backup(&journal.backup);
        if self.error.is_none() {
            self.dismiss_journal();
        }
    }

    fn assist_view(&self) -> AssistView {
        let (running, probe_error) = match ensure_assist_closed(self.backend.as_ref()) {
            Ok(()) => (Vec::new(), None),
            Err(GuardError::AssistRunning(names)) => (names, None),
            Err(GuardError::Probe(e)) => (Vec::new(), Some(e)),
        };
        let (linked, config_error) = match self.assist_config.as_deref().map(assist_config::linked_layers) {
            None => (Vec::new(), None),
            Some(Ok(l)) => (
                l.into_iter()
                    .map(|l| LinkedLayerView {
                        layer: l.layer,
                        profile: l.profile,
                    })
                    .collect(),
                None,
            ),
            Some(Err(e)) => (Vec::new(), Some(e)),
        };
        AssistView {
            running,
            probe_error,
            linked,
            config_error,
        }
    }

    fn journal_view(&self) -> Option<JournalView> {
        let j: Journal = self.journal.pending().ok().flatten()?;
        let conn = self.conn.as_ref()?;
        let ed = conn.editor.as_ref()?;
        Some(JournalView {
            started: j.started.clone(),
            backup: j.backup.to_string_lossy().into_owned(),
            layers: j
                .classify(ed.base())
                .into_iter()
                .map(|(layer, s)| JournalLayerView {
                    layer,
                    state: match s {
                        LayerState::Original => "original",
                        LayerState::Target => "target",
                        LayerState::Unknown => "unknown",
                    }
                    .to_owned(),
                })
                .collect(),
        })
    }

    fn change_views(&self, model: &ModelDescriptor, changes: &[SlotChange]) -> Vec<ChangeView> {
        changes
            .iter()
            .map(|c| {
                let key = model.key_by_index(c.index);
                ChangeView {
                    layer: c.layer,
                    index: c.index,
                    key: key.map(|k| k.id.clone()),
                    legend: key.map(|k| k.legend.clone()),
                    old: SlotView::new(c.old, false, self.lang),
                    new: SlotView::new(c.new, true, self.lang),
                }
            })
            .collect()
    }

    pub fn snapshot(&self) -> Snapshot {
        let devices: Vec<DeviceView> = self.found.iter().map(|f| self.device_view(f)).collect();
        let device = self
            .conn
            .as_ref()
            .and_then(|c| self.found.iter().find(|f| f.info.path == c.device.info().path))
            .map(|f| self.device_view(f));
        let assist = self.assist_view();
        let model = self.model();
        let editor = self.conn.as_ref().and_then(|c| c.editor.as_ref());
        let mut layers = Vec::new();
        let mut changes = Vec::new();
        if let (Some(m), Some(ed)) = (model, editor) {
            let a = m.addressing();
            for l in 0..ed.working().len() {
                layers.push(
                    m.layout
                        .keys
                        .iter()
                        .map(|k| {
                            let now = ed.working().slot(a, l, k.index).unwrap_or_default();
                            let was = ed.base().slot(a, l, k.index).unwrap_or_default();
                            SlotView::new(now, now != was, self.lang)
                        })
                        .collect(),
                );
            }
            changes = self.change_views(m, &ed.changes(m));
        }
        let phase = if self.conn.is_none() {
            // A supported keyboard that is listed but not connected (it stopped answering, or opening it failed)
            // is not "unsupported": the error banner says what happened.
            if !self.found.is_empty() && self.found.iter().all(|f| f.model.is_none()) {
                Phase::Unsupported
            } else {
                Phase::NoDevice
            }
        } else if editor.is_some() {
            Phase::Ready
        } else if self.read_failed {
            Phase::ReadFailed
        } else {
            Phase::Connected
        };
        let plan = match (&self.plan, model) {
            (Some(p), Some(m)) => Some(PlanView {
                layers: p.layers.clone(),
                changes: self.change_views(m, &p.changes),
                backup: p.backup.to_string_lossy().into_owned(),
                linked: assist
                    .linked
                    .iter()
                    .filter(|l| p.layers.contains(&l.layer))
                    .cloned()
                    .collect(),
                experimental: m.support == Support::Experimental,
            }),
            _ => None,
        };
        let report = self.report.as_ref().map(|r| ReportView {
            succeeded: r.succeeded(),
            consistent: r.consistent(),
            outcomes: r
                .outcomes
                .iter()
                .map(|(layer, o)| OutcomeView {
                    layer: *layer,
                    outcome: match o {
                        LayerOutcome::Written => "written",
                        LayerOutcome::NotWritten => "not_written",
                        LayerOutcome::Restored => "restored",
                        LayerOutcome::RestoreFailed => "restore_failed",
                        LayerOutcome::Unknown => "unknown",
                    }
                    .to_owned(),
                })
                .collect(),
            error: r.error.clone(),
            backup: r.backup.to_string_lossy().into_owned(),
            journal_kept: r.journal_kept,
        });
        Snapshot {
            phase,
            lang: self.lang,
            devices,
            device,
            model_id: model.map(|m| m.id.clone()),
            layers,
            changes,
            can_undo: editor.is_some_and(Editor::can_undo),
            can_redo: editor.is_some_and(Editor::can_redo),
            assist,
            journal: self.journal_view(),
            plan,
            report,
            firmware: self.firmware_view(model),
            tap_hold_upgrade: self.tap_hold_upgrade(),
            fw: self.fw_view(),
            error: self.error.clone(),
            simulated: self.simulated,
        }
    }

    /// Whether the firmware on the keyboard reads tap-hold keys from the layer slots.
    fn layer_tap_hold(&self) -> bool {
        self.conn
            .as_ref()
            .and_then(|c| c.info.as_ref())
            .is_some_and(|i| i.layer_tap_hold == Some(type_code::TAP_HOLD))
    }

    /// The layers hold tap-hold slots the firmware on the keyboard does not read yet.
    fn tap_hold_upgrade(&self) -> bool {
        let (Some(m), Some(ed)) = (self.model(), self.conn.as_ref().and_then(|c| c.editor.as_ref())) else {
            return false;
        };
        !self.layer_tap_hold() && has_tap_hold_slot(m, ed.working())
    }

    /// The connected firmware's info block as a view, or `None` when there is no connection, model or block.
    fn firmware_view(&self, model: Option<&ModelDescriptor>) -> Option<FirmwareView> {
        let info = self.conn.as_ref()?.info.as_ref()?;
        Some(FirmwareView::from_info(info, model?))
    }
}

#[cfg(test)]
mod tests {
    use filco_core::Page;
    use filco_protocol::CMD_WRITE_PAGE;
    use filco_service::backend::SimBackend;
    use filco_sim::{Fault, FaultKind, SimKeyboard, Stage};

    use crate::view::FwEditInput;

    use super::*;

    fn rig() -> (App, SimBackend, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let sim = SimBackend::new(SimKeyboard::new(vec![Page::empty(); 4]));
        let paths = Paths {
            backups: tmp.path().join("backups"),
            state: tmp.path().to_owned(),
            assist_config: Some(tmp.path().join("filco_assist_config.json")),
            firmware: Some(repo_firmware()),
        };
        let app = App::new(Arc::new(sim.clone()), &paths, true);
        (app, sim, tmp)
    }

    fn repo_firmware() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../firmware")
    }

    fn key(mods: u8, usage: u8) -> Action {
        Action::Key { mods, usage }
    }

    #[test]
    fn connects_reads_edits_writes() {
        let (mut app, sim, _tmp) = rig();
        let mut steps = Vec::new();
        app.refresh(&mut |p| steps.push(p.step));
        let s = app.snapshot();
        assert_eq!(s.phase, Phase::Ready);
        assert_eq!(s.layers.len(), 4);
        assert_eq!(s.layers[0].len(), 72);
        assert_eq!(steps.last(), Some(&4));

        app.assign(&[2], 27, &key(0, 0x13));
        let s = app.snapshot();
        assert!(s.error.is_none(), "{:?}", s.error);
        assert_eq!(s.changes.len(), 1);
        assert_eq!(s.changes[0].key.as_deref(), Some("P"));
        let p = s.layers[2].iter().find(|v| v.changed).unwrap();
        assert_eq!(p.hex, "02 00 13 00");

        app.prepare_write(&mut |_| {});
        let plan = app.snapshot().plan.unwrap();
        assert!(plan.experimental);
        assert_eq!(plan.layers, vec![2]);
        assert_eq!(sim.with(|k| k.flash_writes()), 0);
        app.execute_write(&mut |_| {});
        let s = app.snapshot();
        assert!(s.report.as_ref().unwrap().succeeded);
        assert_eq!(s.changes.len(), 0);
        assert!(!s.can_undo);
        assert_eq!(sim.with(|k| k.flash_writes()), 1);
        assert_eq!(app.backups().len(), 1);
    }

    fn info_block_of(build: &str) -> [u8; 512] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/firmware")
            .join(build)
            .join("image.bin");
        let image = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        image[0xE600..0xE600 + 512].try_into().unwrap()
    }

    #[test]
    fn reads_and_shows_the_firmware_info_block() {
        let tmp = tempfile::tempdir().unwrap();
        let sim = SimBackend::new(
            SimKeyboard::new(vec![Page::empty(); 4]).with_info_block(info_block_of("HJKL_CapsSpace2_Info")),
        );
        let paths = Paths {
            backups: tmp.path().join("backups"),
            state: tmp.path().to_owned(),
            assist_config: Some(tmp.path().join("filco_assist_config.json")),
            firmware: Some(repo_firmware()),
        };
        let mut app = App::new(Arc::new(sim), &paths, true);
        app.refresh(&mut |_| {});

        let fw = app
            .snapshot()
            .firmware
            .expect("a patched keyboard reports its config");
        assert_eq!(fw.name, "HJKL_CapsSpace2_Info");
        // CapsLock key-hold and two Space layer-holds.
        assert_eq!(fw.tap_holds.len(), 3);
        let caps = fw
            .tap_holds
            .iter()
            .find(|t| !t.hold_is_layer)
            .expect("one key-hold");
        assert_eq!((caps.tap.as_str(), caps.hold.as_str()), ("Esc", "Ctrl"));
        let space_l3 = fw.tap_holds.iter().find(|t| t.hold == "\u{2192} L3").unwrap();
        assert!(space_l3.hold_is_layer);
        assert_eq!(space_l3.tap, "Space");
        // H/J/K/L remapped, I removed (code None).
        assert_eq!(fw.fn_edits.len(), 5);
        assert!(
            fw.fn_edits
                .iter()
                .any(|e| e.legend.as_deref() == Some("I") && e.code.is_none())
        );
    }

    fn app_with_info(block: &[u8; 512]) -> (App, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let sim = SimBackend::new(SimKeyboard::new(vec![Page::empty(); 4]).with_info_block(*block));
        let paths = Paths {
            backups: tmp.path().join("backups"),
            state: tmp.path().to_owned(),
            assist_config: Some(tmp.path().join("filco_assist_config.json")),
            firmware: Some(repo_firmware()),
        };
        let mut app = App::new(Arc::new(sim), &paths, true);
        app.refresh(&mut |_| {});
        (app, tmp)
    }

    const ESC_CTRL: Action = Action::TapHold {
        tap: 0x29,
        hold: 0xE0,
        term: 0,
    };

    #[test]
    fn tap_hold_slots_on_stock_firmware_ask_for_the_update() {
        let (mut app, _sim, _tmp) = rig();
        app.refresh(&mut |_| {});
        assert!(!app.snapshot().tap_hold_upgrade);
        app.assign(&[0], 0x20, &ESC_CTRL);
        let s = app.snapshot();
        assert!(s.error.is_none(), "{:?}", s.error);
        assert_eq!(s.changes.len(), 1);
        assert!(
            s.tap_hold_upgrade,
            "saving must flash a firmware that reads the slot"
        );
        app.undo();
        assert!(!app.snapshot().tap_hold_upgrade);
    }

    #[test]
    fn firmware_tap_holds_move_into_the_slots_with_the_first_tap_hold() {
        let (mut app, _tmp) = app_with_info(&info_block_of("HJKL_CapsSpace2_Info"));
        let (caps, semi) = (4, 29);
        let s = app.snapshot();
        assert_eq!(s.firmware.as_ref().map(|f| f.tap_holds.len()), Some(3));
        // A plain key elsewhere: nothing moves.
        app.assign(&[0], 0x20, &key(0, 0x13));
        assert_eq!(app.snapshot().changes.len(), 1);
        app.undo();
        app.assign(&[0], semi, &ESC_CTRL);
        let s = app.snapshot();
        assert!(s.error.is_none(), "{:?}", s.error);
        assert!(s.tap_hold_upgrade);
        let order: Vec<u8> = app.model().unwrap().layout.keys.iter().map(|k| k.index).collect();
        let pos = |index: u8| order.iter().position(|&k| k == index).unwrap();
        for l in 0..4 {
            assert_eq!(
                s.layers[l][pos(caps)].hex,
                "08 29 e0 14",
                "Caps in layer {}",
                l + 1
            );
        }
        let spaces: Vec<&str> = s.layers[0]
            .iter()
            .filter(|v| v.hold_layer.is_some())
            .map(|v| v.hex.as_str())
            .collect();
        assert_eq!(
            spaces,
            ["08 2c f2 1e", "08 2c f3 1e"],
            "both Space keys, layers 3 and 4"
        );
        assert_eq!(s.changes.len(), 1 + 3 * 4);
        // One undo step takes it all back.
        app.undo();
        assert_eq!(app.snapshot().changes.len(), 0);
        // Editing one of those keys moves them too; the edited slot keeps the user's value.
        app.assign(&[1], caps, &key(0, 0x39));
        let s = app.snapshot();
        assert_eq!(s.layers[1][pos(caps)].hex, "02 00 39 00");
        assert_eq!(s.layers[0][pos(caps)].hex, "08 29 e0 14");
    }

    #[test]
    fn a_keyboard_with_layer_tap_hold_takes_tap_hold_slots_as_they_are() {
        let block = filco_firmware::info::block(&filco_firmware::info::BlockContent {
            name: "LayerTap".into(),
            layer_tap_hold: true,
            ..Default::default()
        })
        .unwrap();
        let (mut app, _tmp) = app_with_info(&block.try_into().unwrap());
        let s = app.snapshot();
        assert!(s.firmware.as_ref().is_some_and(|f| f.layer_tap_hold));
        assert!(s.fw.as_ref().is_some_and(|f| f.layer_tap_hold && f.changes == 0));
        // The same key gets its own tap-hold in two layers.
        app.assign(&[0], 0x20, &ESC_CTRL);
        app.assign(
            &[1],
            0x20,
            &Action::TapHold {
                tap: 0x39,
                hold: 0xF2,
                term: 30,
            },
        );
        let s = app.snapshot();
        assert!(s.error.is_none(), "{:?}", s.error);
        assert!(!s.tap_hold_upgrade);
        let (l1, l2) = (&s.layers[0], &s.layers[1]);
        let i = l1.iter().position(|v| v.changed).unwrap();
        assert_eq!(
            (l1[i].kind.as_str(), l1[i].hex.as_str()),
            ("tap_hold", "08 29 e0 00")
        );
        assert_eq!(
            (l1[i].short.as_str(), l1[i].cap_sub.as_deref()),
            ("Esc", Some("Ctrl"))
        );
        assert_eq!((l2[i].hex.as_str(), l2[i].hold_layer), ("08 39 f2 1e", Some(2)));
        assert_eq!(s.changes.len(), 2);
    }

    /// Edit the draft, build it for real, "flash" it with a simulated updater, and check after the keyboard is
    /// back: the whole firmware flow without a keyboard.
    #[test]
    fn firmware_edit_build_flash_and_check() {
        let tmp = tempfile::tempdir().unwrap();
        let sim = SimBackend::new(
            SimKeyboard::new(vec![Page::empty(); 4]).with_info_block(info_block_of("HJKL_CapsSpace2_Info")),
        );
        let paths = Paths {
            backups: tmp.path().join("backups"),
            state: tmp.path().to_owned(),
            assist_config: None,
            firmware: Some(repo_firmware()),
        };
        let mut app = App::new(Arc::new(sim.clone()), &paths, true);
        let s2 = sim.clone();
        app.set_launcher(Box::new(move |dir: &Path| {
            let image = std::fs::read(dir.join("image.bin")).map_err(|e| e.to_string())?;
            s2.with(|k| {
                k.set_info_block(Some(image[0xE600..0xE600 + 512].try_into().unwrap()));
                k.unplug();
            });
            Ok(())
        }));
        app.refresh(&mut |_| {});
        let fw = app.snapshot().fw.expect("firmware mode after the read");
        assert_eq!(fw.on_keyboard.as_deref(), Some("HJKL_CapsSpace2_Info"));
        assert_eq!(fw.on_keyboard_tag, Some(0x1321));
        assert_eq!((fw.changes, fw.fn_slots_used), (0, 39));
        assert_eq!(fw.keys.iter().filter(|k| k.tap_hold.is_some()).count(), 3);
        assert_eq!(fw.suggested_name, "HJKL_CapsSpace2_Info_2");

        // Fn + O = Home instead of PrintScreen; the draft is autosaved.
        let o = 19;
        app.fw_edit(&FwEditInput::SetFn {
            key: o,
            code: Some(0x4A),
        });
        let fw = app.snapshot().fw.unwrap();
        assert_eq!(fw.changes, 1);
        let ok = fw.keys.iter().find(|k| k.index == o).unwrap();
        assert!(ok.changed && ok.fn_code.as_ref().unwrap().legend == "Home");
        assert!(tmp.path().join(filco_service::firmware::DRAFT_FILE).is_file());
        // A refused edit says why and changes nothing.
        app.fw_edit(&FwEditInput::SetFn {
            key: 10,
            code: Some(0x04),
        });
        assert_eq!(app.snapshot().error.unwrap().code, "fw_edit_refused");
        assert_eq!(app.snapshot().fw.unwrap().changes, 1);

        let job = app.fw_start_build("Home_O").unwrap();
        let done = job.run(&mut |_| {});
        app.fw_finish_build(&job, done);
        let b = app.snapshot().fw.unwrap().build.unwrap();
        assert_eq!(b.passed, Some(true), "{:?}", b.failures);
        assert!(b.has_updater && !b.stale);
        assert_eq!(b.steps.last().map(|s| s.id.as_str()), Some("updater"));
        assert_eq!(app.fw_builds().len(), 1);

        app.fw_flash_prepare();
        assert!(app.snapshot().error.is_none(), "{:?}", app.snapshot().error);
        assert_eq!(app.snapshot().fw.unwrap().flash.unwrap().step, "ready");
        app.fw_flash_launch();
        assert_eq!(app.snapshot().fw.unwrap().flash.unwrap().step, "waiting");
        app.refresh(&mut |_| {});
        assert!(app.snapshot().fw.is_none(), "keyboard gone while flashing");
        sim.with(SimKeyboard::reconnect);
        app.refresh(&mut |_| {});
        let fw = app.snapshot().fw.unwrap();
        let f = fw.flash.unwrap();
        assert_eq!(f.step, "done");
        assert_eq!(f.firmware_ok, Some(true));
        assert_eq!(f.layers_changed, Vec::<usize>::new());
        assert_eq!(fw.on_keyboard.as_deref(), Some("Home_O"));
        assert_eq!(fw.changes, 0, "the keyboard now runs the draft");
        assert!(app.fw_builds()[0].on_keyboard);
    }

    /// A keyboard read first on stock firmware, then with a patched one (reconnected): a draft without edits
    /// follows the new firmware instead of showing its differences as edits.
    #[test]
    fn an_unedited_draft_follows_a_new_firmware() {
        let (mut app, sim, _tmp) = rig();
        app.refresh(&mut |_| {});
        assert_eq!(app.snapshot().fw.unwrap().changes, 0);
        sim.with(|k| {
            k.set_info_block(Some(info_block_of("HJKL_CapsSpace2_Info")));
            k.unplug();
        });
        app.refresh(&mut |_| {});
        sim.with(SimKeyboard::reconnect);
        app.refresh(&mut |_| {});
        let fw = app.snapshot().fw.unwrap();
        assert_eq!(fw.on_keyboard.as_deref(), Some("HJKL_CapsSpace2_Info"));
        assert_eq!(fw.changes, 0);
        assert_eq!(fw.keys.iter().filter(|k| k.tap_hold.is_some()).count(), 3);
    }

    #[test]
    fn a_build_that_is_stale_cannot_be_flashed() {
        let (mut app, _sim, _tmp) = rig();
        app.refresh(&mut |_| {});
        let fw = app.snapshot().fw.unwrap();
        assert_eq!(fw.on_keyboard, None, "stock firmware has no info block");
        app.fw_edit(&FwEditInput::SetFn {
            key: 19,
            code: Some(0x4A),
        });
        let job = app.fw_start_build("x").unwrap();
        let done = job.run(&mut |_| {});
        app.fw_finish_build(&job, done);
        app.fw_edit(&FwEditInput::SetFn {
            key: 27,
            code: Some(0x4A),
        });
        assert!(app.snapshot().fw.unwrap().build.unwrap().stale);
        app.fw_flash_prepare();
        assert_eq!(app.snapshot().error.unwrap().code, "fw_not_flashable");
        assert!(app.fw_start_build("bad name").is_none());
        assert_eq!(app.snapshot().error.unwrap().code, "fw_bad_name");
    }

    #[test]
    fn stock_firmware_has_no_info_in_the_snapshot() {
        let (mut app, _sim, _tmp) = rig();
        app.refresh(&mut |_| {});
        assert_eq!(app.snapshot().phase, Phase::Ready);
        assert!(
            app.snapshot().firmware.is_none(),
            "stock firmware has no info block"
        );
    }

    #[test]
    fn assist_running_blocks_reading_and_writing() {
        let (mut app, sim, _tmp) = rig();
        sim.set_processes(&["FilcoAssistUI.exe"]);
        app.refresh(&mut |_| {});
        let s = app.snapshot();
        assert_eq!(s.phase, Phase::Connected);
        assert_eq!(s.assist.running, vec!["FilcoAssistUI.exe"]);
        app.read(&mut |_| {});
        assert_eq!(app.snapshot().error.unwrap().code, "assist_running");
        sim.set_processes(&[]);
        app.read(&mut |_| {});
        assert_eq!(app.snapshot().phase, Phase::Ready);
    }

    #[test]
    fn unplug_and_replug() {
        let (mut app, sim, _tmp) = rig();
        app.refresh(&mut |_| {});
        sim.with(SimKeyboard::unplug);
        app.refresh(&mut |_| {});
        assert_eq!(app.snapshot().phase, Phase::NoDevice);
        sim.with(SimKeyboard::reconnect);
        app.refresh(&mut |_| {});
        assert_eq!(app.snapshot().phase, Phase::Ready);
    }

    #[test]
    fn failed_write_reports_and_rereads() {
        let (mut app, sim, _tmp) = rig();
        app.refresh(&mut |_| {});
        app.assign(&[1, 3], 70, &key(0, 0xa4));
        app.prepare_write(&mut |_| {});
        sim.with(|k| {
            k.add_fault(Fault {
                cmd: CMD_WRITE_PAGE,
                nth: 1,
                stage: Stage::Finish,
                kind: FaultKind::WrongAck,
            });
        });
        app.execute_write(&mut |_| {});
        let s = app.snapshot();
        let r = s.report.unwrap();
        assert!(!r.succeeded && r.consistent);
        let o: Vec<_> = r.outcomes.iter().map(|o| o.outcome.as_str()).collect();
        assert_eq!(o, vec!["restored", "restored"]);
        // The edits are still there, ready to try again.
        assert_eq!(s.changes.len(), 2);
    }

    #[test]
    fn unfinished_write_is_reported_on_next_connection() {
        let (mut app, sim, tmp) = rig();
        app.refresh(&mut |_| {});
        app.assign(&[0, 1], 27, &key(0, 0x13));
        app.prepare_write(&mut |_| {});
        sim.with(|k| {
            k.add_fault(Fault {
                cmd: CMD_WRITE_PAGE,
                nth: 1,
                stage: Stage::Data { n: 4 },
                kind: FaultKind::Disconnect,
            });
        });
        app.execute_write(&mut |_| {});
        let s = app.snapshot();
        assert!(s.report.unwrap().journal_kept);
        assert_eq!(
            s.phase,
            Phase::NoDevice,
            "a supported keyboard that went away is not 'unsupported'"
        );
        sim.with(SimKeyboard::reconnect);

        // A new app session (restart): the journal is found and explained.
        let paths = Paths {
            backups: tmp.path().join("backups"),
            state: tmp.path().to_owned(),
            assist_config: None,
            firmware: Some(repo_firmware()),
        };
        let mut app = App::new(Arc::new(sim.clone()), &paths, true);
        app.refresh(&mut |_| {});
        let j = app.snapshot().journal.unwrap();
        let states: Vec<_> = j.layers.iter().map(|l| l.state.as_str()).collect();
        assert_eq!(states, vec!["target", "original"]);
        app.prepare_write(&mut |_| {});
        app.restore_from_journal();
        let s = app.snapshot();
        assert!(s.journal.is_none());
        assert_eq!(s.changes.len(), 1, "layer 0 goes back to the backup");
        app.prepare_write(&mut |_| {});
        app.execute_write(&mut |_| {});
        assert!(app.snapshot().report.unwrap().succeeded);
        assert_eq!(sim.with(|k| k.flash().to_vec()), vec![Page::empty(); 4]);
    }

    #[test]
    fn invalid_edits_and_import_export_round_trip() {
        let (mut app, _sim, tmp) = rig();
        app.refresh(&mut |_| {});
        app.assign(
            &[0],
            27,
            &Action::Raw {
                hex: "06 01 00 00".into(),
            },
        );
        assert_eq!(app.snapshot().error.unwrap().code, "edit_invalid");
        app.assign(&[0], 27, &key(0x08, 0x16));
        let file = tmp.path().join("a.filco-layers.json");
        app.export_file(&file);
        app.revert();
        assert_eq!(app.snapshot().changes.len(), 0);
        app.import_file(&file);
        let s = app.snapshot();
        assert_eq!(s.changes.len(), 1);
        app.undo();
        assert_eq!(app.snapshot().changes.len(), 0);
        app.redo();
        assert_eq!(app.snapshot().changes.len(), 1);
    }

    #[test]
    fn linked_layers_from_filco_assist_settings() {
        let (mut app, _sim, tmp) = rig();
        std::fs::write(
            tmp.path().join("filco_assist_config.json"),
            r#"{"K":{"Layers":[{"LayerNo":1,"ProfileUuid":"u"}],"Profiles":[{"ProfileUuid":"u","Name":"s1"}]}}"#,
        )
        .unwrap();
        app.refresh(&mut |_| {});
        app.assign(&[1], 27, &key(0, 0x13));
        app.prepare_write(&mut |_| {});
        let plan = app.snapshot().plan.unwrap();
        assert_eq!(
            plan.linked,
            vec![LinkedLayerView {
                layer: 1,
                profile: "s1".into()
            }]
        );
    }
}
