//! The ZIP archive inside the updater: read it, and write it back as Python's `zipfile` writes it (the updaters
//! built that way by the analysis repository were flashed successfully, so their layout is the one kept).
//!
//! Writer layout, per entry: local header (version 20, flags, deflate, the entry's DOS time and date, CRC,
//! sizes, name, no extra field), the data, then the central directory (made by version 20 on MS-DOS, external
//! attributes 0o600 << 16) and the end record. Names are read as `zipfile` reads them (CP437 unless flag 0x800
//! says UTF-8, `\` turned into `/`) and written ASCII, or UTF-8 with flag 0x800.
//!
//! An entry the build does not change keeps the stock compressed bytes as they are; only new data is compressed
//! (deflate level 6, as `zipfile`). Compressed bytes of untouched entries therefore do not depend on the deflate
//! implementation at hand.

use std::io::{Read, Write};

use crate::Error;

/// One archive entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Name as `zipfile` gives it (`/` separators).
    pub name: String,
    pub method: u16,
    pub time: u16,
    pub date: u16,
    pub crc: u32,
    pub size: u32,
    /// Compressed bytes, exactly as stored.
    pub raw: Vec<u8>,
}

impl Entry {
    /// The uncompressed data.
    pub fn data(&self) -> Result<Vec<u8>, Error> {
        let out = match self.method {
            0 => self.raw.clone(),
            8 => {
                let mut v = Vec::with_capacity(self.size as usize);
                flate2::read::DeflateDecoder::new(&self.raw[..])
                    .read_to_end(&mut v)
                    .map_err(|e| Error::Zip(e.to_string()))?;
                v
            }
            m => {
                return Err(Error::Zip(format!(
                    "{}: compression method {m} not supported",
                    self.name
                )));
            }
        };
        if out.len() != self.size as usize || crc32(&out) != self.crc {
            return Err(Error::Zip(format!("{}: CRC or size mismatch", self.name)));
        }
        Ok(out)
    }

    /// A new deflated entry with the given name and data, keeping `like`'s time and date.
    pub fn deflated(name: &str, data: &[u8], like: &Self) -> Result<Self, Error> {
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
        enc.write_all(data).map_err(|e| Error::Zip(e.to_string()))?;
        let raw = enc.finish().map_err(|e| Error::Zip(e.to_string()))?;
        Ok(Self {
            name: name.to_string(),
            method: 8,
            time: like.time,
            date: like.date,
            crc: crc32(data),
            size: u32::try_from(data.len()).map_err(|_| Error::Zip("entry too large".into()))?,
            raw,
        })
    }
}

/// CRC-32 (IEEE), as ZIP uses it.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// CP437 code points of bytes 0x80-0xFF.
const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

fn cp437(bytes: &[u8]) -> String {
    let high: Vec<char> = CP437_HIGH.chars().collect();
    bytes
        .iter()
        .map(|&b| {
            if b < 0x80 {
                char::from(b)
            } else {
                high[usize::from(b - 0x80)]
            }
        })
        .collect()
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, Error> {
    b.get(at..at + 2)
        .map(|x| u16::from_le_bytes([x[0], x[1]]))
        .ok_or_else(|| Error::Zip("truncated".into()))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, Error> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .ok_or_else(|| Error::Zip("truncated".into()))
}

/// Reads every entry, in central-directory order.
pub fn read(z: &[u8]) -> Result<Vec<Entry>, Error> {
    let eocd = (0..z.len().saturating_sub(21))
        .rev()
        .find(|&i| z[i..i + 4] == [0x50, 0x4B, 5, 6])
        .ok_or_else(|| Error::Zip("no end of central directory".into()))?;
    let count = usize::from(u16_at(z, eocd + 10)?);
    let mut p = u32_at(z, eocd + 16)? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if z.get(p..p + 4) != Some(&[0x50, 0x4B, 1, 2]) {
            return Err(Error::Zip("bad central directory entry".into()));
        }
        let flags = u16_at(z, p + 8)?;
        let (nlen, xlen, clen) = (u16_at(z, p + 28)?, u16_at(z, p + 30)?, u16_at(z, p + 32)?);
        let raw_name = z
            .get(p + 46..p + 46 + usize::from(nlen))
            .ok_or_else(|| Error::Zip("truncated".into()))?;
        let name = if flags & 0x800 != 0 {
            String::from_utf8(raw_name.to_vec()).map_err(|_| Error::Zip("bad UTF-8 name".into()))?
        } else {
            cp437(raw_name)
        }
        .replace('\\', "/");
        let csize = u32_at(z, p + 20)? as usize;
        let local = u32_at(z, p + 42)? as usize;
        if z.get(local..local + 4) != Some(&[0x50, 0x4B, 3, 4]) {
            return Err(Error::Zip(format!("{name}: bad local header")));
        }
        let start = local + 30 + usize::from(u16_at(z, local + 26)?) + usize::from(u16_at(z, local + 28)?);
        let raw = z
            .get(start..start + csize)
            .ok_or_else(|| Error::Zip(format!("{name}: data out of range")))?;
        out.push(Entry {
            name,
            method: u16_at(z, p + 10)?,
            time: u16_at(z, p + 12)?,
            date: u16_at(z, p + 14)?,
            crc: u32_at(z, p + 16)?,
            size: u32_at(z, p + 24)?,
            raw: raw.to_vec(),
        });
        p += 46 + usize::from(nlen) + usize::from(xlen) + usize::from(clen);
    }
    Ok(out)
}

fn name_bytes(name: &str) -> (Vec<u8>, u16) {
    if name.is_ascii() {
        (name.as_bytes().to_vec(), 0)
    } else {
        (name.as_bytes().to_vec(), 0x800)
    }
}

/// Writes the archive.
pub fn write(entries: &[Entry]) -> Result<Vec<u8>, Error> {
    let too_big = || Error::Zip("archive too large".into());
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let offset = u32::try_from(out.len()).map_err(|_| too_big())?;
        let (name, flags) = name_bytes(&e.name);
        let nlen = u16::try_from(name.len()).map_err(|_| too_big())?;
        let csize = u32::try_from(e.raw.len()).map_err(|_| too_big())?;
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&flags.to_le_bytes());
            v.extend_from_slice(&e.method.to_le_bytes());
            v.extend_from_slice(&e.time.to_le_bytes());
            v.extend_from_slice(&e.date.to_le_bytes());
            v.extend_from_slice(&e.crc.to_le_bytes());
            v.extend_from_slice(&csize.to_le_bytes());
            v.extend_from_slice(&e.size.to_le_bytes());
            v.extend_from_slice(&nlen.to_le_bytes());
        };
        out.extend_from_slice(&[0x50, 0x4B, 3, 4, 20, 0]);
        common(&mut out);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&name);
        out.extend_from_slice(&e.raw);
        central.extend_from_slice(&[0x50, 0x4B, 1, 2, 20, 0, 20, 0]);
        common(&mut central);
        central.extend_from_slice(&[0; 8]); // extra, comment, disk, internal attributes
        central.extend_from_slice(&(0o600u32 << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(&name);
    }
    let cd_offset = u32::try_from(out.len()).map_err(|_| too_big())?;
    let cd_size = u32::try_from(central.len()).map_err(|_| too_big())?;
    let n = u16::try_from(entries.len()).map_err(|_| too_big())?;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4B, 5, 6, 0, 0, 0, 0]);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_cp437() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(CP437_HIGH.chars().count(), 128);
        assert_eq!(cp437(&[0xA5, 0xDF, 0xC4]), "Ñ▀─");
    }

    #[test]
    fn round_trip() {
        let like = Entry {
            name: String::new(),
            method: 8,
            time: 0,
            date: 0,
            crc: 0,
            size: 0,
            raw: vec![],
        };
        let a = Entry::deflated("a/b.txt", b"hello hello hello", &like).unwrap();
        let b = Entry::deflated("Ñ.zip", b"", &like).unwrap();
        let z = write(&[a.clone(), b.clone()]).unwrap();
        let back = read(&z).unwrap();
        assert_eq!(back, vec![a, b]);
        assert_eq!(back[0].data().unwrap(), b"hello hello hello");
    }
}
