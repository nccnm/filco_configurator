//! Intel HEX for 64 KB SN32F24xB images (port of `hexfile.py`).
//!
//! [`HexFile::parse`] returns the image (bytes not covered by any record read as 0xFF) and keeps the original
//! lines. [`HexFile::write_like`] writes a new image back with the original file as a template: every line stays
//! as it is except data records whose bytes changed, which get the new data and a recomputed checksum. Record
//! layout, CRLF line endings and the covered address set never change.

use std::fmt::Write as _;

use thiserror::Error;

/// Size of the flash image.
pub const SIZE: usize = 0x10000;

/// A 64 KB image.
pub type Image = Vec<u8>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HexError {
    #[error("line {0}: not an Intel HEX record")]
    Syntax(usize),
    #[error("line {0}: bad checksum")]
    Checksum(usize),
    #[error("HEX has data above 64 KB")]
    TooLarge,
    #[error("changes outside the HEX records, first at {0:#06x}")]
    Uncovered(usize),
    #[error("HEX written does not read back as the new image")]
    ReadBack,
}

#[derive(Debug, Clone)]
struct Record {
    line: usize,
    kind: u8,
    addr: usize,
    bytes: Vec<u8>,
}

/// A parsed HEX file: its lines (template for writing) and which addresses it covers.
#[derive(Debug, Clone)]
pub struct HexFile {
    lines: Vec<String>,
    covered: Vec<bool>,
}

fn records(lines: &[String]) -> Result<Vec<Record>, HexError> {
    let mut base = 0usize;
    let mut out = Vec::new();
    for (i, ln) in lines.iter().enumerate() {
        let Some(hex) = ln.strip_prefix(':') else { continue };
        if hex.len() % 2 != 0 || hex.len() < 10 {
            return Err(HexError::Syntax(i + 1));
        }
        let b = (0..hex.len())
            .step_by(2)
            .map(|k| u8::from_str_radix(&hex[k..k + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| HexError::Syntax(i + 1))?;
        if b.iter().fold(0u8, |a, &x| a.wrapping_add(x)) != 0 {
            return Err(HexError::Checksum(i + 1));
        }
        if b.len() != usize::from(b[0]) + 5 {
            return Err(HexError::Syntax(i + 1));
        }
        let addr = usize::from(b[1]) << 8 | usize::from(b[2]);
        let kind = b[3];
        match kind {
            4 => base = (usize::from(b[4]) << 8 | usize::from(b[5])) << 16,
            2 => base = (usize::from(b[4]) << 8 | usize::from(b[5])) << 4,
            _ => {}
        }
        out.push(Record {
            line: i,
            kind,
            addr: base + addr,
            bytes: b,
        });
    }
    Ok(out)
}

impl HexFile {
    /// Parses HEX text; returns the template and the image.
    pub fn parse(text: &str) -> Result<(Self, Image), HexError> {
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let mut img = vec![0xFF; SIZE];
        let mut covered = vec![false; SIZE];
        for r in records(&lines)? {
            if r.kind != 0 {
                continue;
            }
            for k in 0..usize::from(r.bytes[0]) {
                let a = r.addr + k;
                if a >= SIZE {
                    return Err(HexError::TooLarge);
                }
                img[a] = r.bytes[4 + k];
                covered[a] = true;
            }
        }
        Ok((Self { lines, covered }, img))
    }

    /// The HEX text of `new_img`, laid out like this file. `old_img` is the image this file holds.
    pub fn write_like(&self, old_img: &[u8], new_img: &[u8]) -> Result<String, HexError> {
        if let Some(a) = (0..SIZE).find(|&a| old_img[a] != new_img[a] && !self.covered[a]) {
            return Err(HexError::Uncovered(a));
        }
        let mut out = self.lines.clone();
        for r in records(&self.lines)? {
            if r.kind != 0 {
                continue;
            }
            let n = usize::from(r.bytes[0]);
            let data = &new_img[r.addr..r.addr + n];
            if &r.bytes[4..4 + n] != data {
                let mut b = r.bytes.clone();
                b[4..4 + n].copy_from_slice(data);
                let last = b.len() - 1;
                b[last] = b[..last].iter().fold(0u8, |a, &x| a.wrapping_sub(x));
                out[r.line] = b.iter().fold(String::from(":"), |mut s, x| {
                    let _ = write!(s, "{x:02X}");
                    s
                });
            }
        }
        let mut text = out.join("\r\n");
        text.push_str("\r\n");
        let (check, img) = Self::parse(&text)?;
        if img != new_img || check.covered != self.covered {
            return Err(HexError::ReadBack);
        }
        Ok(text)
    }
}

/// 16-bit sum of the image as little-endian words.
pub fn word_sum(img: &[u8]) -> u16 {
    img.as_chunks::<2>()
        .0
        .iter()
        .fold(0u16, |a, w| a.wrapping_add(u16::from_le_bytes(*w)))
}
