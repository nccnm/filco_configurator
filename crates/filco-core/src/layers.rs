//! All layer pages of a keyboard and the slot-level difference between two sets.

use serde::Serialize;

use crate::addressing::SlotAddressing;
use crate::page::Page;
use crate::slot::Slot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerSet {
    pages: Vec<Page>,
}

/// One slot that differs between two layer sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SlotChange {
    pub layer: usize,
    pub index: u8,
    pub old: Slot,
    pub new: Slot,
}

impl LayerSet {
    pub fn new(pages: Vec<Page>) -> Self {
        LayerSet { pages }
    }

    pub fn empty(layers: usize) -> Self {
        LayerSet {
            pages: vec![Page::empty(); layers],
        }
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn page(&self, layer: usize) -> Option<&Page> {
        self.pages.get(layer)
    }

    pub fn page_mut(&mut self, layer: usize) -> Option<&mut Page> {
        self.pages.get_mut(layer)
    }

    pub fn slot(&self, addressing: SlotAddressing, layer: usize, index: u8) -> Option<Slot> {
        self.pages.get(layer)?.slot(addressing, index)
    }

    /// Every slot that differs from `self` (old) to `other` (new), by layer then matrix index.
    ///
    /// # Panics
    /// If the two sets do not have the same number of layers.
    pub fn diff(&self, other: &LayerSet, addressing: SlotAddressing) -> Vec<SlotChange> {
        assert_eq!(self.len(), other.len(), "layer sets of different sizes");
        let mut out = Vec::new();
        for (layer, (a, b)) in self.pages.iter().zip(&other.pages).enumerate() {
            if a == b {
                continue;
            }
            for index in 0..u8::try_from(addressing.slot_count()).unwrap_or(u8::MAX) {
                let (old, new) = (a.slot(addressing, index), b.slot(addressing, index));
                if let (Some(old), Some(new)) = (old, new)
                    && old != new
                {
                    out.push(SlotChange {
                        layer,
                        index,
                        old,
                        new,
                    });
                }
            }
        }
        out
    }

    /// Layers whose pages differ.
    pub fn changed_layers(&self, other: &LayerSet) -> Vec<usize> {
        self.pages
            .iter()
            .zip(&other.pages)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: SlotAddressing = SlotAddressing::CostarInterleaved8;

    #[test]
    fn diff_lists_changed_slots_in_order() {
        let base = LayerSet::empty(4);
        let mut edited = base.clone();
        let fn_slot: Slot = "02 00 a4 00".parse().unwrap();
        let p: Slot = "02 0a 16 00".parse().unwrap();
        edited.page_mut(3).unwrap().set_slot(A, 27, p);
        edited.page_mut(0).unwrap().set_slot(A, 70, fn_slot);
        edited.page_mut(0).unwrap().set_slot(A, 0, fn_slot);
        let d = base.diff(&edited, A);
        assert_eq!(
            d,
            vec![
                SlotChange {
                    layer: 0,
                    index: 0,
                    old: Slot::EMPTY,
                    new: fn_slot
                },
                SlotChange {
                    layer: 0,
                    index: 70,
                    old: Slot::EMPTY,
                    new: fn_slot
                },
                SlotChange {
                    layer: 3,
                    index: 27,
                    old: Slot::EMPTY,
                    new: p
                },
            ]
        );
        assert_eq!(base.changed_layers(&edited), vec![0, 3]);
        assert_eq!(base.diff(&base, A), vec![]);
    }
}
