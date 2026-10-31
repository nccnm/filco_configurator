//! Lower-case hex text for bytes, as used in files and in the UI.

use std::fmt::Write as _;

/// `[0x02, 0x0a]` -> `"020a"`.
pub fn encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// `[0x02, 0x0a]` -> `"02 0a"`.
pub fn encode_spaced(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Parses hex digits, ignoring ASCII whitespace; case-insensitive.
pub fn decode(text: &str) -> Result<Vec<u8>, HexError> {
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return Err(HexError::OddLength);
    }
    digits
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[hi, lo]| Ok((nibble(*hi)? << 4) | nibble(*lo)?))
        .collect()
}

fn nibble(c: u8) -> Result<u8, HexError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(HexError::BadDigit(char::from(c))),
    }
}

/// Parses `"0x2EFD"` (or plain hex digits) into a number of at most 16 bits.
pub fn parse_u16(text: &str) -> Result<u16, HexError> {
    let t = text.trim();
    let digits = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    if digits.is_empty() || digits.len() > 4 {
        return Err(HexError::Range(t.to_owned()));
    }
    u16::from_str_radix(digits, 16).map_err(|_| HexError::Range(t.to_owned()))
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HexError {
    #[error("hex text has an odd number of digits")]
    OddLength,
    #[error("not a hex digit: {0:?}")]
    BadDigit(char),
    #[error("not a 16-bit hex number: {0:?}")]
    Range(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let bytes = [0x00, 0x02, 0x0a, 0xff];
        assert_eq!(encode(&bytes), "00020aff");
        assert_eq!(encode_spaced(&bytes), "00 02 0a ff");
        assert_eq!(decode("00 02 0A ff").unwrap(), bytes);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(decode("0"), Err(HexError::OddLength));
        assert_eq!(decode("0g"), Err(HexError::BadDigit('g')));
        assert!(parse_u16("0x10000").is_err());
        assert!(parse_u16("0x").is_err());
        assert_eq!(parse_u16("0x2EFD").unwrap(), 0x2efd);
        assert_eq!(parse_u16("1972").unwrap(), 0x1972);
    }
}
