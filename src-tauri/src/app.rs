//! Application state behind the Tauri commands. No Tauri types here, so it is tested like any other code.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use filco_core::model::{KnownDevices, Support, builtin};
use filco_core::{LayerSet, ModelDescriptor, SlotChange, export};
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

/// Where the app keeps its files.
#[derive(Debug, Clone)]
pub struct Paths {
    pub backups: PathBuf,
    /// Folder of `journal.json`.
    pub state: PathBuf,
    /// FILCO Assist's settings file (read only), if it can exist on this machine.
    pub assist_config: Option<PathBuf>,
}

impl Paths {
    pub fn default_for_user() -> Option<Self> {
        let backups = BackupStore::default_root()?;
        let state = backups.parent()?.to_owned();
        Some(Paths {
            backups,
            state,
            assist_config: assist_config::default_path(),
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
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("found", &self.found.len())
            .finish_non_exhaustive()
    }
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
        }
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

    pub fn assign(&mut self, layers: &[usize], index: u8, action: &Action) {
        self.edit(|m, ed| {
            let slot = action.to_slot().map_err(|e| ErrorView::new("edit_invalid", e))?;
            ed.assign_layers(m, layers, index, slot)
                .map(|_| ())
                .map_err(|e| ErrorView::new("edit_invalid", e))
        });
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
            error: self.error.clone(),
            simulated: self.simulated,
        }
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

    use super::*;

    fn rig() -> (App, SimBackend, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let sim = SimBackend::new(SimKeyboard::new(vec![Page::empty(); 4]));
        let paths = Paths {
            backups: tmp.path().join("backups"),
            state: tmp.path().to_owned(),
            assist_config: Some(tmp.path().join("filco_assist_config.json")),
        };
        let app = App::new(Arc::new(sim.clone()), &paths, true);
        (app, sim, tmp)
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
