//! Where the slot of a key matrix index lives inside a layer page.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Slot layout of a page. A model names one of these; nothing else in the code computes offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SlotAddressing {
    /// Costar / FILCO firmware (0x6842-0x6870 in V60): slot of index k at `0x40 * (k & 7) + 4 * (k >> 3)`,
    /// 128 slots in a 512-byte page.
    #[serde(rename = "costar-interleaved-8")]
    CostarInterleaved8,
}

impl SlotAddressing {
    /// Number of slots this layout addresses.
    pub const fn slot_count(self) -> usize {
        match self {
            SlotAddressing::CostarInterleaved8 => 128,
        }
    }

    /// Byte offset of the slot for matrix `index`, or `None` if the index is out of range.
    pub const fn offset(self, index: u8) -> Option<usize> {
        match self {
            SlotAddressing::CostarInterleaved8 => {
                if index < 128 {
                    Some(0x40 * (index as usize & 7) + 4 * (index as usize >> 3))
                } else {
                    None
                }
            }
        }
    }

    /// Matrix index whose slot starts at `offset`, if any.
    pub fn index_at(self, offset: usize) -> Option<u8> {
        (0..=u8::MAX).find(|&i| self.offset(i) == Some(offset))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const A: SlotAddressing = SlotAddressing::CostarInterleaved8;

    #[test]
    fn verified_offsets() {
        // FN_ANALYSIS.md section 2 and the configs/ READMEs of the analysis repository.
        assert_eq!(A.offset(70), Some(0x1a0)); // M10
        assert_eq!(A.offset(73), Some(0x064)); // Fn
        assert_eq!(A.offset(27), Some(0x0cc)); // P
        assert_eq!(A.offset(83), Some(0x0e8)); // left Alt, KeyIndex 232 in FILCO Assist
        assert_eq!(A.offset(56), Some(0x01c)); // M1, hard-coded offset in FILCO Assist
        assert_eq!(A.offset(128), None);
    }

    #[test]
    fn bijection_onto_aligned_offsets() {
        let offsets: HashSet<usize> = (0..128).map(|i| A.offset(i).unwrap()).collect();
        assert_eq!(offsets.len(), 128);
        assert!(offsets.iter().all(|o| o % 4 == 0 && *o < 512));
        for i in 0..128u8 {
            assert_eq!(A.index_at(A.offset(i).unwrap()), Some(i));
        }
    }
}
