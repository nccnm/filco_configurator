//! The editing session: the pages as last read from the keyboard (base), the pages being edited (working),
//! and undo / redo. Every change goes through here, checked against the model.

use filco_core::slot::type_code;
use filco_core::{LayerSet, ModelDescriptor, MouseAction, Slot, SlotChange, SlotKind};
use serde::Serialize;

/// One undoable step: the slots it changed, with their values before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Edit {
    changes: Vec<SlotChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum EditError {
    #[error("layer {0} does not exist on this model")]
    NoLayer(usize),
    #[error("matrix index {0} has no key on this model")]
    NoKey(u8),
    #[error("slot type {0} cannot be edited on this model")]
    NotEditable(u8),
    #[error("{0}")]
    Invalid(String),
    #[error("the pages do not belong to this model")]
    Shape,
}

#[derive(Debug, Clone)]
pub struct Editor {
    base: LayerSet,
    working: LayerSet,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

/// Checks that a slot is something the user may write on `model`.
pub fn check_slot(model: &ModelDescriptor, slot: Slot) -> Result<(), EditError> {
    let kind = slot.kind();
    let code = match kind {
        SlotKind::Empty => type_code::EMPTY,
        SlotKind::Key { mods, usage } => {
            if usage == 0 && mods.0 == 0 {
                return Err(EditError::Invalid("a keystroke needs a key or a modifier".into()));
            }
            type_code::KEY
        }
        SlotKind::Mouse { action } => {
            let ok = match action {
                MouseAction::Click { buttons } => buttons != 0 && buttons & !0x1f == 0,
                MouseAction::Scroll { amount } => amount != 0,
                MouseAction::Move { dx, dy } => dx != 0 || dy != 0,
            };
            if !ok {
                return Err(EditError::Invalid("this mouse action does nothing".into()));
            }
            type_code::MOUSE
        }
        SlotKind::Consumer { usage } => {
            if usage == 0 {
                return Err(EditError::Invalid("a media action needs a usage".into()));
            }
            type_code::CONSUMER
        }
        SlotKind::System { usage } => {
            if usage == 0 || usage & !0x07 != 0 {
                return Err(EditError::Invalid(
                    "system control is a mask of 0x01, 0x02, 0x04".into(),
                ));
            }
            type_code::SYSTEM
        }
        SlotKind::TapHold { tap, hold, .. } => {
            if tap == 0 || hold == 0 {
                return Err(EditError::Invalid(
                    "a tap-hold key needs a tap key and a hold".into(),
                ));
            }
            type_code::TAP_HOLD
        }
        SlotKind::Macro { .. } => type_code::MACRO,
        SlotKind::Other { bytes } => bytes[0] & 0x0f,
    };
    if matches!(kind, SlotKind::Other { .. }) || !model.is_editable_type(code) {
        return Err(EditError::NotEditable(code));
    }
    Ok(())
}

impl Editor {
    /// Starts editing the pages just read from the keyboard.
    pub fn new(model: &ModelDescriptor, base: LayerSet) -> Result<Self, EditError> {
        if base.len() != model.layers.len() {
            return Err(EditError::Shape);
        }
        Ok(Editor {
            working: base.clone(),
            base,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn base(&self) -> &LayerSet {
        &self.base
    }

    pub fn working(&self) -> &LayerSet {
        &self.working
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Slots that differ between the keyboard (base) and the edited pages.
    pub fn changes(&self, model: &ModelDescriptor) -> Vec<SlotChange> {
        self.base.diff(&self.working, model.addressing())
    }

    pub fn is_dirty(&self) -> bool {
        self.base != self.working
    }

    fn apply(&mut self, model: &ModelDescriptor, edits: &[(usize, u8, Slot)]) -> Result<bool, EditError> {
        let a = model.addressing();
        let mut changes = Vec::new();
        for &(layer, index, slot) in edits {
            if layer >= self.working.len() {
                return Err(EditError::NoLayer(layer));
            }
            let old = self
                .working
                .slot(a, layer, index)
                .ok_or(EditError::NoKey(index))?;
            if old != slot
                && !changes
                    .iter()
                    .any(|c: &SlotChange| c.layer == layer && c.index == index)
            {
                changes.push(SlotChange {
                    layer,
                    index,
                    old,
                    new: slot,
                });
            }
        }
        if changes.is_empty() {
            return Ok(false);
        }
        for c in &changes {
            if let Some(p) = self.working.page_mut(c.layer) {
                p.set_slot(a, c.index, c.new);
            }
        }
        self.undo.push(Edit { changes });
        self.redo.clear();
        Ok(true)
    }

    /// Sets one slot. Returns false if it already had that value.
    pub fn assign(
        &mut self,
        model: &ModelDescriptor,
        layer: usize,
        index: u8,
        slot: Slot,
    ) -> Result<bool, EditError> {
        if model.key_by_index(index).is_none() {
            return Err(EditError::NoKey(index));
        }
        check_slot(model, slot)?;
        self.apply(model, &[(layer, index, slot)])
    }

    /// Sets the same slot of one key on several layers as one undo step.
    pub fn assign_layers(
        &mut self,
        model: &ModelDescriptor,
        layers: &[usize],
        index: u8,
        slot: Slot,
    ) -> Result<bool, EditError> {
        if model.key_by_index(index).is_none() {
            return Err(EditError::NoKey(index));
        }
        check_slot(model, slot)?;
        let edits: Vec<_> = layers.iter().map(|&l| (l, index, slot)).collect();
        self.apply(model, &edits)
    }

    /// Sets several `(layer, index, slot)` as one undo step; the first edit of a slot wins.
    pub fn assign_many(
        &mut self,
        model: &ModelDescriptor,
        edits: &[(usize, u8, Slot)],
    ) -> Result<bool, EditError> {
        for &(_, index, slot) in edits {
            if model.key_by_index(index).is_none() {
                return Err(EditError::NoKey(index));
            }
            check_slot(model, slot)?;
        }
        self.apply(model, edits)
    }

    /// Copies every slot of layer `from` onto layer `to` as one undo step.
    pub fn copy_layer(&mut self, model: &ModelDescriptor, from: usize, to: usize) -> Result<bool, EditError> {
        let src = self.working.page(from).ok_or(EditError::NoLayer(from))?.clone();
        if to >= self.working.len() {
            return Err(EditError::NoLayer(to));
        }
        let a = model.addressing();
        let edits: Vec<_> = (0..u8::try_from(a.slot_count()).unwrap_or(u8::MAX))
            .filter_map(|i| src.slot(a, i).map(|s| (to, i, s)))
            .collect();
        self.apply(model, &edits)
    }

    /// Replaces the edited pages with `pages` (an imported file or a backup) as one undo step. Slots the editor
    /// could not produce itself (macros, unknown types) are accepted here: they come from a real keyboard.
    pub fn replace_all(&mut self, model: &ModelDescriptor, pages: &LayerSet) -> Result<bool, EditError> {
        if pages.len() != self.working.len() {
            return Err(EditError::Shape);
        }
        let changes = self.working.diff(pages, model.addressing());
        let edits: Vec<_> = changes.iter().map(|c| (c.layer, c.index, c.new)).collect();
        self.apply(model, &edits)
    }

    pub fn undo(&mut self, model: &ModelDescriptor) -> bool {
        let Some(edit) = self.undo.pop() else { return false };
        let a = model.addressing();
        for c in edit.changes.iter().rev() {
            if let Some(p) = self.working.page_mut(c.layer) {
                p.set_slot(a, c.index, c.old);
            }
        }
        self.redo.push(edit);
        true
    }

    pub fn redo(&mut self, model: &ModelDescriptor) -> bool {
        let Some(edit) = self.redo.pop() else { return false };
        let a = model.addressing();
        for c in &edit.changes {
            if let Some(p) = self.working.page_mut(c.layer) {
                p.set_slot(a, c.index, c.new);
            }
        }
        self.undo.push(edit);
        true
    }

    /// Discards every edit.
    pub fn revert(&mut self) {
        self.working = self.base.clone();
        self.undo.clear();
        self.redo.clear();
    }

    /// After a successful write: what the keyboard now holds becomes the new base, edits are kept as the new
    /// working pages if they differ (they should not).
    pub fn rebase(&mut self, device: LayerSet) {
        let clean = device == self.working;
        self.base = device;
        if clean {
            self.undo.clear();
            self.redo.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use filco_core::model::builtin;

    use super::*;

    fn model() -> ModelDescriptor {
        builtin::models().unwrap().remove(0)
    }

    fn s(t: &str) -> Slot {
        t.parse().unwrap()
    }

    #[test]
    fn assign_undo_redo() {
        let m = model();
        let mut e = Editor::new(&m, LayerSet::empty(4)).unwrap();
        assert!(e.assign(&m, 2, 27, s("02 0a 16 00")).unwrap());
        assert!(
            !e.assign(&m, 2, 27, s("02 0a 16 00")).unwrap(),
            "same value is not a new step"
        );
        assert!(e.assign(&m, 2, 27, s("02 00 13 00")).unwrap());
        assert_eq!(e.changes(&m).len(), 1);
        assert!(e.undo(&m));
        assert_eq!(e.working().slot(m.addressing(), 2, 27), Some(s("02 0a 16 00")));
        assert!(e.undo(&m));
        assert!(!e.is_dirty());
        assert!(!e.undo(&m));
        assert!(e.redo(&m));
        assert!(e.redo(&m));
        assert_eq!(e.working().slot(m.addressing(), 2, 27), Some(s("02 00 13 00")));
        assert!(!e.redo(&m));
        // A new edit clears redo.
        e.undo(&m);
        e.assign(&m, 0, 0, s("02 00 29 00")).unwrap();
        assert!(!e.can_redo());
    }

    #[test]
    fn rejects_what_the_model_does_not_allow() {
        let m = model();
        let mut e = Editor::new(&m, LayerSet::empty(4)).unwrap();
        assert_eq!(e.assign(&m, 4, 27, s("02 00 04 00")), Err(EditError::NoLayer(4)));
        assert_eq!(
            e.assign(&m, 0, 53, s("02 00 04 00")),
            Err(EditError::NoKey(53)),
            "53 has no physical key"
        );
        assert_eq!(
            e.assign(&m, 0, 27, s("06 01 00 00")),
            Err(EditError::NotEditable(6))
        );
        assert_eq!(
            e.assign(&m, 0, 27, s("07 00 00 00")),
            Err(EditError::NotEditable(7))
        );
        assert!(matches!(
            e.assign(&m, 0, 27, s("02 00 00 00")),
            Err(EditError::Invalid(_))
        ));
        assert!(matches!(
            e.assign(&m, 0, 27, s("01 01 00 00")),
            Err(EditError::Invalid(_))
        ));
        assert!(matches!(
            e.assign(&m, 0, 27, s("04 82 00 00")),
            Err(EditError::Invalid(_))
        ));
        assert!(matches!(
            e.assign(&m, 0, 27, s("03 00 00 00")),
            Err(EditError::Invalid(_))
        ));
        assert!(!e.is_dirty());
        for ok in [
            "00 00 00 00",
            "02 07 00 00",
            "01 03 ff 00",
            "01 04 05 fb",
            "03 e9 00 00",
            "04 02 00 00",
        ] {
            e.assign(&m, 0, 27, s(ok))
                .unwrap_or_else(|err| panic!("{ok}: {err}"));
        }
    }

    #[test]
    fn multi_layer_copy_and_replace_are_single_steps() {
        let m = model();
        let mut e = Editor::new(&m, LayerSet::empty(4)).unwrap();
        e.assign_layers(&m, &[0, 1, 2, 3], 70, s("02 00 a4 00")).unwrap();
        assert_eq!(e.changes(&m).len(), 4);
        e.assign(&m, 1, 0, s("02 00 35 00")).unwrap();
        e.copy_layer(&m, 1, 3).unwrap();
        assert_eq!(e.working().page(1), e.working().page(3));
        e.undo(&m);
        assert_ne!(e.working().page(1), e.working().page(3));
        let mut other = LayerSet::empty(4);
        other
            .page_mut(0)
            .unwrap()
            .set_slot(m.addressing(), 5, s("06 01 00 00"));
        e.replace_all(&m, &other).unwrap();
        assert_eq!(e.working(), &other);
        e.undo(&m);
        e.undo(&m);
        e.undo(&m);
        assert!(!e.is_dirty());
    }

    #[test]
    fn rebase_after_write() {
        let m = model();
        let mut e = Editor::new(&m, LayerSet::empty(4)).unwrap();
        e.assign(&m, 0, 27, s("02 00 13 00")).unwrap();
        let written = e.working().clone();
        e.rebase(written);
        assert!(!e.is_dirty());
        assert!(!e.can_undo());
    }
}
