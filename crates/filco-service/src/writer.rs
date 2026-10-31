//! The only way pages reach the keyboard.
//!
//! 1. [`prepare`]: FILCO Assist closed, no unfinished write, the model allows writing; read every layer fresh
//!    and compare with what the editor started from (someone else may have changed the keyboard); save a backup
//!    of the fresh pages. The returned [`WritePlan`] holds exactly the pages to write and the slot changes the
//!    user confirms.
//! 2. [`execute`] (after the user confirmed the diff): FILCO Assist still closed; write the journal; write each
//!    changed layer and read it back byte for byte. On the first failure, put every layer already written and the
//!    failing one back to the backed-up pages, and say per layer what it holds.

use std::path::PathBuf;

use filco_core::backup::BackupError;
use filco_core::{LayerSet, ModelDescriptor, SlotChange};
use filco_protocol::{Progress, ProtocolError, TransportError};
use serde::Serialize;

use crate::device::Device;
use crate::editor::Editor;
use crate::guard::{GuardError, ProcessProbe, ensure_assist_closed};
use crate::journal::{Journal, JournalError, JournalFile};
use crate::store::BackupStore;

/// What a write needs besides the device.
#[allow(missing_debug_implementations)]
pub struct WriteContext<'a> {
    pub store: &'a BackupStore,
    pub journal: &'a JournalFile,
    pub probe: &'a dyn ProcessProbe,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WritePlan {
    pub model_id: String,
    pub device_path: String,
    /// Layers to write, ascending.
    pub layers: Vec<usize>,
    /// Every changed slot, for the confirmation dialog.
    pub changes: Vec<SlotChange>,
    /// Backup of `original`.
    pub backup: PathBuf,
    #[serde(skip)]
    pub original: LayerSet,
    #[serde(skip)]
    pub target: LayerSet,
}

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error(transparent)]
    Guard(#[from] GuardError),
    #[error("this model is read-only in this version")]
    ReadOnlyModel,
    #[error("an earlier write did not finish; deal with it first ({0})")]
    JournalPending(PathBuf),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error("there is nothing to write")]
    NothingToWrite,
    #[error("reading the keyboard before writing failed: {0}")]
    Read(#[source] ProtocolError),
    /// The keyboard no longer holds what the editor started from. `pages` is what it holds now.
    #[error("the keyboard changed since it was read (layers {layers:?})")]
    DeviceChanged { layers: Vec<usize>, pages: LayerSet },
    #[error("could not save the backup: {0}")]
    Backup(#[from] BackupError),
}

/// Checks, reads fresh, compares, backs up. Nothing is written to the keyboard.
pub fn prepare(
    device: &mut Device,
    model: &ModelDescriptor,
    editor: &Editor,
    ctx: &WriteContext<'_>,
    created: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<WritePlan, PrepareError> {
    if !model.can_write() {
        return Err(PrepareError::ReadOnlyModel);
    }
    ensure_assist_closed(ctx.probe)?;
    if ctx.journal.pending()?.is_some() {
        return Err(PrepareError::JournalPending(ctx.journal.path().to_owned()));
    }
    let target = editor.working().clone();
    let layers = editor.base().changed_layers(&target);
    if layers.is_empty() {
        return Err(PrepareError::NothingToWrite);
    }
    let fresh = device.read_all(progress).map_err(PrepareError::Read)?;
    if &fresh != editor.base() {
        return Err(PrepareError::DeviceChanged {
            layers: editor.base().changed_layers(&fresh),
            pages: fresh,
        });
    }
    let backup = ctx
        .store
        .save(model, device.info(), &fresh, "before-write", created)?;
    Ok(WritePlan {
        model_id: model.id.clone(),
        device_path: device.info().path.clone(),
        layers,
        changes: editor.changes(model),
        backup,
        original: fresh,
        target,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerOutcome {
    /// Holds the new page (read back and compared).
    Written,
    /// Never touched.
    NotWritten,
    /// Was (partly) written, then put back to the backed-up page (read back and compared).
    Restored,
    /// Putting the backed-up page back failed; content unknown.
    RestoreFailed,
    /// The keyboard went away during this layer; content unknown.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WriteReport {
    pub outcomes: Vec<(usize, LayerOutcome)>,
    /// The first error, if the write failed.
    pub error: Option<String>,
    pub backup: PathBuf,
    /// The journal stays when a layer is in an unknown state, so the next connection reports it.
    pub journal_kept: bool,
}

impl WriteReport {
    pub fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    /// True when every layer holds either the old or the new page, all of them the same one.
    pub fn consistent(&self) -> bool {
        let all = |o: &[LayerOutcome]| self.outcomes.iter().all(|(_, x)| o.contains(x));
        all(&[LayerOutcome::Written]) || all(&[LayerOutcome::Restored, LayerOutcome::NotWritten])
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    #[error(transparent)]
    Guard(#[from] GuardError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error("the plan was made for another keyboard")]
    WrongDevice,
}

fn is_gone(e: &ProtocolError) -> bool {
    matches!(e, ProtocolError::Transport(TransportError::Disconnected))
}

/// Writes the plan. Errors are returned only for checks that fail before anything is written; once writing
/// started, the outcome of every layer is in the report.
pub fn execute(
    device: &mut Device,
    plan: &WritePlan,
    ctx: &WriteContext<'_>,
    started: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<WriteReport, ExecuteError> {
    if device.info().path != plan.device_path {
        return Err(ExecuteError::WrongDevice);
    }
    ensure_assist_closed(ctx.probe)?;
    let journal = Journal::new(
        &plan.model_id,
        &plan.device_path,
        started,
        &plan.backup,
        &plan.layers,
        &plan.original,
        &plan.target,
    );
    ctx.journal.begin(&journal)?;

    let mut outcomes: Vec<(usize, LayerOutcome)> = plan
        .layers
        .iter()
        .map(|&l| (l, LayerOutcome::NotWritten))
        .collect();
    let error = device.with_writes(|s| {
        let mut failure = None;
        for (i, &layer) in plan.layers.iter().enumerate() {
            let Some(page) = plan.target.page(layer) else {
                continue;
            };
            let l = u8::try_from(layer).unwrap_or(u8::MAX);
            match s.write_page(l, page, progress) {
                Ok(()) => outcomes[i].1 = LayerOutcome::Written,
                Err(e) => {
                    failure = Some((i, e));
                    break;
                }
            }
        }
        let (failed, err) = failure?;
        if is_gone(&err) {
            outcomes[failed].1 = LayerOutcome::Unknown;
            return Some(err);
        }
        // Put back the failing layer first, then the written ones.
        let mut gone = false;
        for i in std::iter::once(failed).chain((0..failed).rev()) {
            let layer = outcomes[i].0;
            if gone {
                if outcomes[i].1 != LayerOutcome::Written {
                    outcomes[i].1 = LayerOutcome::Unknown;
                }
                continue;
            }
            let Some(page) = plan.original.page(layer) else {
                continue;
            };
            let l = u8::try_from(layer).unwrap_or(u8::MAX);
            match s.write_page(l, page, progress) {
                Ok(()) => outcomes[i].1 = LayerOutcome::Restored,
                Err(e) => {
                    gone = is_gone(&e);
                    outcomes[i].1 = if gone {
                        LayerOutcome::Unknown
                    } else {
                        LayerOutcome::RestoreFailed
                    };
                }
            }
        }
        Some(err)
    });

    let report_ok = match &error {
        None => true,
        Some(_) => outcomes
            .iter()
            .all(|(_, o)| matches!(o, LayerOutcome::Restored | LayerOutcome::NotWritten)),
    };
    // A journal that cannot be removed is reported, not hidden: the next connection will check the layers.
    let journal_kept = !report_ok || ctx.journal.finish().is_err();
    Ok(WriteReport {
        outcomes,
        error: error.map(|e| e.to_string()),
        backup: plan.backup.clone(),
        journal_kept,
    })
}
