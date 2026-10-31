//! Data model shared by every part of the editor: the 4-byte key slot, the 512-byte layer page, layer sets and
//! their differences, keyboard model descriptors, and the backup / export file formats.
//!
//! Nothing here talks to a device. Byte layouts follow `firmware/FN_ANALYSIS.md` sections 2-3 and
//! `firmware/tools/fnmap.py` of the analysis repository.
#![forbid(unsafe_code)]

pub mod addressing;
pub mod backup;
pub mod export;
pub mod hex;
pub mod label;
pub mod layers;
pub mod model;
pub mod page;
pub mod slot;
pub mod usage;

pub use addressing::SlotAddressing;
pub use layers::{LayerSet, SlotChange};
pub use model::ModelDescriptor;
pub use page::{PAGE_LEN, Page};
pub use slot::{ModMask, MouseAction, Slot, SlotKind};
