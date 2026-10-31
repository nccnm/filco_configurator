//! One layer page: the 512 bytes the firmware stores at `0xf000 + 0x240 * layer`.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::addressing::SlotAddressing;
use crate::hex;
use crate::slot::Slot;

pub const PAGE_LEN: usize = 512;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Page(Box<[u8; PAGE_LEN]>);

impl Page {
    /// An all-zero page (every slot empty).
    pub fn empty() -> Self {
        Page(Box::new([0; PAGE_LEN]))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PageLenError> {
        let arr: [u8; PAGE_LEN] = bytes.try_into().map_err(|_| PageLenError(bytes.len()))?;
        Ok(Page(Box::new(arr)))
    }

    pub fn as_bytes(&self) -> &[u8; PAGE_LEN] {
        &self.0
    }

    /// The slot of matrix `index`; `None` if the addressing has no slot for it.
    pub fn slot(&self, addressing: SlotAddressing, index: u8) -> Option<Slot> {
        let off = addressing.offset(index)?;
        let mut b = [0u8; 4];
        b.copy_from_slice(&self.0[off..off + 4]);
        Some(Slot(b))
    }

    /// Replaces the slot of matrix `index`; returns the previous slot, or `None` (and changes nothing) if the
    /// addressing has no slot for it.
    pub fn set_slot(&mut self, addressing: SlotAddressing, index: u8, slot: Slot) -> Option<Slot> {
        let off = addressing.offset(index)?;
        let old = self.slot(addressing, index);
        self.0[off..off + 4].copy_from_slice(&slot.0);
        old
    }

    /// SHA-256 of the page as lower-case hex, the digest `fnmap.py` stores in its manifest.
    pub fn sha256_hex(&self) -> String {
        hex::encode(&Sha256::digest(self.0.as_slice()))
    }
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let used = self.0.as_chunks::<4>().0.iter().filter(|c| **c != [0; 4]).count();
        write!(
            f,
            "Page({used} non-empty slots, sha256 {})",
            &self.sha256_hex()[..12]
        )
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("a layer page is {PAGE_LEN} bytes, got {0}")]
pub struct PageLenError(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    const A: SlotAddressing = SlotAddressing::CostarInterleaved8;

    #[test]
    fn slot_access() {
        let mut p = Page::empty();
        let fn_slot: Slot = "02 00 a4 00".parse().unwrap();
        assert_eq!(p.set_slot(A, 70, fn_slot), Some(Slot::EMPTY));
        assert_eq!(&p.as_bytes()[0x1a0..0x1a4], &[0x02, 0x00, 0xa4, 0x00]);
        assert_eq!(p.slot(A, 70), Some(fn_slot));
        assert_eq!(p.set_slot(A, 200, fn_slot), None);
        assert_eq!(p.as_bytes().iter().filter(|b| **b != 0).count(), 2);
    }

    #[test]
    fn length_is_checked() {
        assert_eq!(Page::from_bytes(&[0; 511]), Err(PageLenError(511)));
        assert!(Page::from_bytes(&[0; 512]).is_ok());
    }

    #[test]
    fn sha256_of_empty_page() {
        // sha256 of 512 zero bytes.
        assert_eq!(
            Page::empty().sha256_hex(),
            "076a27c79e5ace2a3d47f9dd2e83e4ff6ea8872b3c2218f66c92b89b55f36560"
        );
    }
}
