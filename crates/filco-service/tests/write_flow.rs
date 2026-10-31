//! The safe write flow on the simulated keyboard: every failure point of PLAN.md section 5 ends in the stated
//! state, the transport is read-only outside the write, and a backup exists before the first page is written.

use filco_core::model::{Support, builtin};
use filco_core::{LayerSet, ModelDescriptor, Page, Slot};
use filco_protocol::{CMD_READ_PAGE, CMD_WRITE_PAGE, WritePolicy};
use filco_service::backend::SimBackend;
use filco_service::device::Device;
use filco_service::editor::Editor;
use filco_service::guard::GuardError;
use filco_service::journal::{JournalFile, LayerState};
use filco_service::store::{self, BackupStore};
use filco_service::writer::{self, ExecuteError, LayerOutcome, PrepareError, WriteContext, WritePlan};
use filco_sim::{Fault, FaultKind, SimKeyboard, Stage};

fn model() -> ModelDescriptor {
    builtin::models().unwrap().remove(0)
}

fn slot(t: &str) -> Slot {
    t.parse().unwrap()
}

/// The keyboard as the user's real configuration left it (fixture of their own pages).
fn real_pages() -> Vec<Page> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pages/2026-10-31_Layer4Snip");
    store::load(&dir, &model()).unwrap().1.pages().to_vec()
}

struct Rig {
    m: ModelDescriptor,
    backend: SimBackend,
    device: Device,
    editor: Editor,
    store: BackupStore,
    journal: JournalFile,
    _tmp: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        Self::with_sim(SimKeyboard::new(real_pages()))
    }

    fn with_sim(sim: SimKeyboard) -> Self {
        let m = model();
        let backend = SimBackend::new(sim);
        let mut device = Device::open(&backend, &backend.info.clone(), &m).unwrap();
        let base = device.read_all(&mut |_| {}).unwrap();
        let editor = Editor::new(&m, base).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        Rig {
            store: BackupStore::new(tmp.path().join("backups")),
            journal: JournalFile::new(tmp.path()),
            m,
            backend,
            device,
            editor,
            _tmp: tmp,
        }
    }

    /// P becomes Ctrl+Shift+S on layer 2, M10 becomes Mute on layer 4.
    fn edit_two_layers(&mut self) {
        self.editor.assign(&self.m, 1, 27, slot("02 03 16 00")).unwrap();
        self.editor.assign(&self.m, 3, 70, slot("03 e2 00 00")).unwrap();
    }

    fn ctx(&self) -> WriteContext<'_> {
        WriteContext {
            store: &self.store,
            journal: &self.journal,
            probe: &self.backend,
        }
    }

    fn prepare(&mut self) -> Result<WritePlan, PrepareError> {
        let ctx = WriteContext {
            store: &self.store,
            journal: &self.journal,
            probe: &self.backend,
        };
        writer::prepare(
            &mut self.device,
            &self.m,
            &self.editor,
            &ctx,
            "20261101-020000",
            &mut |_| {},
        )
    }

    fn execute(&mut self, plan: &WritePlan) -> Result<writer::WriteReport, ExecuteError> {
        let ctx = WriteContext {
            store: &self.store,
            journal: &self.journal,
            probe: &self.backend,
        };
        writer::execute(&mut self.device, plan, &ctx, "20261101-020001", &mut |_| {})
    }

    fn flash(&self) -> LayerSet {
        LayerSet::new(self.backend.with(|k| k.flash().to_vec()))
    }

    fn writes(&self) -> usize {
        self.backend.with(|k| k.flash_writes())
    }

    /// Faults count sessions since the simulator was created; the rig already read 4 pages, prepare reads 4.
    fn fault(&self, cmd: u8, nth: usize, stage: Stage, kind: FaultKind) {
        self.backend.with(|k| {
            k.add_fault(Fault {
                cmd,
                nth,
                stage,
                kind,
            });
        });
    }
}

fn outcomes(r: &writer::WriteReport) -> Vec<LayerOutcome> {
    r.outcomes.iter().map(|(_, o)| *o).collect()
}

#[test]
fn successful_write_changes_exactly_the_edited_slots() {
    let mut r = Rig::new();
    let before = r.flash();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    assert_eq!(plan.layers, vec![1, 3]);
    assert_eq!(plan.changes.len(), 2);
    assert_eq!(r.writes(), 0, "prepare writes nothing");
    // The backup holds the pages from before, in fnmap.py format.
    let (_, backed_up) = store::load(&plan.backup, &r.m).unwrap();
    assert_eq!(backed_up, before);

    let report = r.execute(&plan).unwrap();
    assert!(report.succeeded() && report.consistent(), "{report:?}");
    assert_eq!(
        outcomes(&report),
        vec![LayerOutcome::Written, LayerOutcome::Written]
    );
    assert!(!report.journal_kept);
    assert!(r.journal.pending().unwrap().is_none());
    assert_eq!(r.writes(), 2);
    assert_eq!(r.flash(), *r.editor.working());
    assert_eq!(before.diff(&r.flash(), r.m.addressing()), plan.changes);
    assert_eq!(r.device.policy(), WritePolicy::ReadOnly);

    r.editor.rebase(plan.target.clone());
    assert!(!r.editor.is_dirty());
}

#[test]
fn reads_and_failed_checks_never_write() {
    let mut r = Rig::new();
    // Read-only outside a write: a write attempt through the device is refused before it reaches the keyboard.
    assert_eq!(r.device.policy(), WritePolicy::ReadOnly);

    assert!(matches!(r.prepare(), Err(PrepareError::NothingToWrite)));

    r.edit_two_layers();
    r.backend.set_processes(&["FilcoAssistTray.exe"]);
    assert!(matches!(
        r.prepare(),
        Err(PrepareError::Guard(GuardError::AssistRunning(_)))
    ));
    r.backend.set_processes(&[]);

    let mut ro = r.m.clone();
    ro.support = Support::ReadOnly;
    let ctx = r.ctx();
    let mut dev = Device::open(&r.backend, &r.backend.info, &ro).unwrap();
    assert!(matches!(
        writer::prepare(&mut dev, &ro, &r.editor, &ctx, "t", &mut |_| {}),
        Err(PrepareError::ReadOnlyModel)
    ));
    assert_eq!(r.writes(), 0);
    assert!(r.store.list(&r.m).is_empty(), "no backup without a write");
}

#[test]
fn keyboard_changed_since_it_was_read() {
    let mut r = Rig::new();
    r.edit_two_layers();
    r.backend
        .with(|k| k.flash_mut()[2].set_slot(r.m.addressing(), 0, slot("02 00 29 00")));
    match r.prepare() {
        Err(PrepareError::DeviceChanged { layers, pages }) => {
            assert_eq!(layers, vec![2]);
            assert_eq!(pages, r.flash());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(r.writes(), 0);
}

#[test]
fn assist_started_between_confirm_and_write() {
    let mut r = Rig::new();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    r.backend.set_processes(&["FilcoAssistUI.exe"]);
    assert!(matches!(r.execute(&plan), Err(ExecuteError::Guard(_))));
    assert_eq!(r.writes(), 0);
    assert!(r.journal.pending().unwrap().is_none());
}

#[test]
fn unfinished_journal_blocks_the_next_write() {
    let mut r = Rig::new();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    std::fs::write(r.journal.path(), "{}").unwrap();
    assert!(matches!(r.execute(&plan), Err(ExecuteError::Journal(_))));
    assert_eq!(r.writes(), 0);
    std::fs::remove_file(r.journal.path()).unwrap();
}

#[test]
fn failure_in_first_layer_puts_it_back() {
    for (stage, kind) in [
        (Stage::Command, FaultKind::WrongAck),
        (Stage::Data { n: 3 }, FaultKind::IoError),
        (Stage::Finish, FaultKind::BadChecksum),
        (Stage::Commit, FaultKind::CorruptCommit),
    ] {
        let mut r = Rig::new();
        let before = r.flash();
        r.edit_two_layers();
        let plan = r.prepare().unwrap();
        r.fault(CMD_WRITE_PAGE, 0, stage, kind);
        let report = r.execute(&plan).unwrap();
        assert!(!report.succeeded(), "{stage:?} {kind:?}");
        assert_eq!(
            outcomes(&report),
            vec![LayerOutcome::Restored, LayerOutcome::NotWritten],
            "{stage:?} {kind:?}"
        );
        assert!(report.consistent());
        assert_eq!(r.flash(), before, "{stage:?} {kind:?}");
        assert!(!report.journal_kept);
        assert_eq!(r.device.policy(), WritePolicy::ReadOnly);
    }
}

#[test]
fn failure_in_second_layer_puts_both_back() {
    let mut r = Rig::new();
    let before = r.flash();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    r.fault(CMD_WRITE_PAGE, 1, Stage::Data { n: 7 }, FaultKind::IoError);
    let report = r.execute(&plan).unwrap();
    assert_eq!(
        outcomes(&report),
        vec![LayerOutcome::Restored, LayerOutcome::Restored]
    );
    assert_eq!(r.flash(), before);
    assert!(report.error.unwrap().contains("HID request failed"));
}

#[test]
fn readback_mismatch_on_verify_read() {
    // The page is stored correctly but the read-back report is corrupted: that is a failed write too, and the
    // restore reads back cleanly.
    let mut r = Rig::new();
    let before = r.flash();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    // Reads so far: 4 by the rig, 4 by prepare. The 9th read is the read-back of layer 1.
    r.fault(CMD_READ_PAGE, 8, Stage::Data { n: 0 }, FaultKind::CorruptData);
    let report = r.execute(&plan).unwrap();
    assert_eq!(outcomes(&report)[0], LayerOutcome::Restored);
    assert_eq!(r.flash(), before);
}

#[test]
fn unplugged_during_second_layer_keeps_the_journal() {
    let mut r = Rig::new();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    r.fault(CMD_WRITE_PAGE, 1, Stage::Data { n: 2 }, FaultKind::Disconnect);
    let report = r.execute(&plan).unwrap();
    assert_eq!(
        outcomes(&report),
        vec![LayerOutcome::Written, LayerOutcome::Unknown]
    );
    assert!(!report.consistent());
    assert!(report.journal_kept);
    assert_eq!(r.device.policy(), WritePolicy::ReadOnly);

    // Plugged in again: the journal says which layers hold what, and the backup restores everything.
    r.backend.with(SimKeyboard::reconnect);
    let journal = r.journal.pending().unwrap().unwrap();
    assert_eq!(journal.backup, plan.backup);
    let mut dev = Device::open(&r.backend, &r.backend.info, &r.m).unwrap();
    let now = dev.read_all(&mut |_| {}).unwrap();
    assert_eq!(
        journal.classify(&now),
        vec![(1, LayerState::Target), (3, LayerState::Original)]
    );

    let (_, backup) = store::load(&journal.backup, &r.m).unwrap();
    let mut ed = Editor::new(&r.m, now).unwrap();
    ed.replace_all(&r.m, &backup).unwrap();
    r.journal.finish().unwrap();
    let ctx = r.ctx();
    let restore = writer::prepare(&mut dev, &r.m, &ed, &ctx, "20261101-030000", &mut |_| {}).unwrap();
    assert_eq!(restore.layers, vec![1]);
    let rep = writer::execute(&mut dev, &restore, &ctx, "20261101-030001", &mut |_| {}).unwrap();
    assert!(rep.succeeded());
    assert_eq!(r.flash(), backup);
}

#[test]
fn restore_that_also_fails_is_reported() {
    let mut r = Rig::new();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    // Write sessions: 0 = layer 1 (ok), 1 = layer 3 (fails), 2 = restore layer 3 (fails), 3 = restore layer 1.
    r.fault(CMD_WRITE_PAGE, 1, Stage::Finish, FaultKind::WrongAck);
    r.fault(CMD_WRITE_PAGE, 2, Stage::Data { n: 1 }, FaultKind::IoError);
    let report = r.execute(&plan).unwrap();
    assert_eq!(
        outcomes(&report),
        vec![LayerOutcome::Restored, LayerOutcome::RestoreFailed]
    );
    assert!(report.journal_kept);
}

#[test]
fn unplugged_during_restore() {
    let mut r = Rig::new();
    r.edit_two_layers();
    let plan = r.prepare().unwrap();
    r.fault(CMD_WRITE_PAGE, 1, Stage::Data { n: 0 }, FaultKind::IoError);
    r.fault(CMD_WRITE_PAGE, 2, Stage::Command, FaultKind::Disconnect);
    let report = r.execute(&plan).unwrap();
    // Layer 1 was written and verified before the failure, so it is known to hold the new page.
    assert_eq!(
        outcomes(&report),
        vec![LayerOutcome::Written, LayerOutcome::Unknown]
    );
    assert!(report.journal_kept);
}

#[test]
fn plan_for_another_device_is_refused() {
    let mut r = Rig::new();
    r.edit_two_layers();
    let mut plan = r.prepare().unwrap();
    plan.device_path = "other".into();
    assert!(matches!(r.execute(&plan), Err(ExecuteError::WrongDevice)));
    assert_eq!(r.writes(), 0);
}

#[test]
fn macros_and_unknown_slots_survive_untouched() {
    // A page with a macro slot and a reserved type: editing another key must keep them byte for byte.
    let mut pages = real_pages();
    let m = model();
    pages[0].set_slot(m.addressing(), 1, slot("06 02 00 00"));
    pages[0].set_slot(m.addressing(), 9, slot("0e 12 34 56"));
    let mut r = Rig::with_sim(SimKeyboard::new(pages.clone()));
    r.editor.assign(&r.m, 0, 27, slot("02 00 13 00")).unwrap();
    let plan = r.prepare().unwrap();
    r.execute(&plan).unwrap();
    let flash = r.flash();
    assert_eq!(flash.slot(r.m.addressing(), 0, 1), Some(slot("06 02 00 00")));
    assert_eq!(flash.slot(r.m.addressing(), 0, 9), Some(slot("0e 12 34 56")));
    assert_eq!(LayerSet::new(pages).diff(&flash, r.m.addressing()).len(), 1);
}
