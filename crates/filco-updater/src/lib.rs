//! Make a copy of the Diatec / Sonix ISP updater that flashes a different HEX (port of `isp_exe.py`).
//!
//! The updater keeps its settings and firmware in a ZIP stored as RT_RCDATA resource 4011. [`build`] copies
//! every entry, swaps the HEX in `Settings/FWFiles` for the new one (stored under its own file name), points
//! every `FWName` in `Settings/UISettings.ini` to it, appends a suffix to the window title, and writes the ZIP
//! into a copy of the exe with the Windows UpdateResource API. The result is read back and compared entry by
//! entry. The updater is never executed here; the user runs it and presses Start.

pub mod zip;

#[cfg(windows)]
mod res;

use std::path::Path;

use thiserror::Error;

/// SHA-256 of the stock updater this was proven with (checked by the caller before building).
pub const STOCK_UPDATER_SHA256: &str = "b3a5f385d5da36f9a9bcc730b286ad6dc82b6e8340cd29e1fcf90fc1eeb641b2";
pub const ZIP_ID: u16 = 4011;
/// The other RT_RCDATA resources, which must stay as they are.
pub const OTHER_RCDATA: [u16; 5] = [4007, 4008, 4009, 4012, 4013];
const FW_DIR: &str = "Settings/FWFiles/";
const INI: &str = "Settings/UISettings.ini";

#[derive(Debug, Error)]
pub enum Error {
    #[error("updater ZIP: {0}")]
    Zip(String),
    #[error("updater resource: {0}")]
    Resource(String),
    #[error("{0}")]
    Io(String),
    #[error("updater check failed: {0}")]
    Check(String),
}

fn utf16_decode(b: &[u8]) -> Result<String, Error> {
    let body = b
        .strip_prefix(&[0xFF, 0xFE][..])
        .ok_or_else(|| Error::Zip("UISettings.ini is not UTF-16 LE".into()))?;
    let units: Vec<u16> = body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16(&units).map_err(|_| Error::Zip("UISettings.ini is not valid UTF-16".into()))
}

fn utf16_encode(s: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    out.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
    out
}

/// Points every `FWName=` line to `hex_name` and appends `suffix` to the first `ISPTitle=` line, exactly as the
/// regular expressions of `isp_exe.py` do (each rewritten line loses its trailing whitespace, CR included).
pub fn rewrite_ini(text: &str, hex_name: &str, suffix: &str) -> Result<String, Error> {
    let mut fw = 0;
    let mut title_done = false;
    let lines: Vec<String> = text
        .split('\n')
        .map(|l| {
            if l.starts_with("FWName=") {
                fw += 1;
                format!("FWName={hex_name}")
            } else if !title_done && l.starts_with("ISPTitle=") {
                title_done = true;
                format!("{}{suffix}", l.trim_end())
            } else {
                l.to_string()
            }
        })
        .collect();
    if fw == 0 {
        return Err(Error::Zip("no FWName in UISettings.ini".into()));
    }
    Ok(lines.join("\n"))
}

/// The new archive: every stock entry in order, the HEX swapped, the INI rewritten. Untouched entries keep
/// their stock compressed bytes.
pub fn rebuild_zip(old: &[u8], hex_name: &str, hex: &[u8], title_suffix: &str) -> Result<Vec<u8>, Error> {
    let entries = zip::read(old)?;
    let mut out = Vec::with_capacity(entries.len());
    let mut replaced = false;
    for e in &entries {
        if e.name.starts_with(FW_DIR) && e.name.to_ascii_lowercase().ends_with(".hex") {
            if replaced {
                return Err(Error::Zip("more than one HEX in Settings/FWFiles".into()));
            }
            replaced = true;
            out.push(zip::Entry::deflated(&format!("{FW_DIR}{hex_name}"), hex, e)?);
        } else if e.name == INI {
            let text = rewrite_ini(&utf16_decode(&e.data()?)?, hex_name, title_suffix)?;
            out.push(zip::Entry::deflated(INI, &utf16_encode(&text), e)?);
        } else {
            out.push(e.clone());
        }
    }
    if !replaced {
        return Err(Error::Zip("no HEX found in Settings/FWFiles".into()));
    }
    zip::write(&out)
}

/// Checks a rebuilt archive against the stock one; returns the INI lines that matter (title, VID, PID, FWName).
pub fn check_zip(new: &[u8], old: &[u8], hex_name: &str, hex: &[u8]) -> Result<Vec<String>, Error> {
    let new_entries = zip::read(new)?;
    let find = |n: &str| new_entries.iter().find(|e| e.name == n);
    let hex_entry = find(&format!("{FW_DIR}{hex_name}")).ok_or_else(|| Error::Check("HEX missing".into()))?;
    if hex_entry.data()? != hex {
        return Err(Error::Check("HEX inside the new exe differs".into()));
    }
    let ini = utf16_decode(
        &find(INI)
            .ok_or_else(|| Error::Check("UISettings.ini missing".into()))?
            .data()?,
    )?;
    let fw_lines = ini.lines().filter(|l| l.starts_with("FWName=")).count();
    if fw_lines == 0 || ini.lines().filter(|l| *l == format!("FWName={hex_name}")).count() != fw_lines {
        return Err(Error::Check("not every FWName points to the new HEX".into()));
    }
    for e in zip::read(old)? {
        if e.name.starts_with(FW_DIR) || e.name == INI {
            continue;
        }
        if find(&e.name).map(zip::Entry::data).transpose()? != Some(e.data()?) {
            return Err(Error::Check(format!("{} changed", e.name)));
        }
    }
    Ok(ini
        .lines()
        .filter(|l| {
            ["ISPTitle=", "FWName=", "VID=", "PID="]
                .iter()
                .any(|p| l.starts_with(p))
        })
        .map(str::to_string)
        .collect())
}

/// Copies `stock_exe` to `out_exe` with the HEX `hex` (file name `hex_name`) and `title_suffix`, reads it back
/// and checks it. Returns the INI lines that matter, for the report.
#[cfg(windows)]
pub fn build(
    stock_exe: &Path,
    hex_name: &str,
    hex: &[u8],
    out_exe: &Path,
    title_suffix: &str,
) -> Result<Vec<String>, Error> {
    let io = |e: std::io::Error| Error::Io(e.to_string());
    let old_zip = res::read(stock_exe, ZIP_ID)?;
    let new_zip = rebuild_zip(&old_zip, hex_name, hex, title_suffix)?;
    std::fs::copy(stock_exe, out_exe).map_err(io)?;
    res::write(out_exe, ZIP_ID, &new_zip)?;
    let back = res::read(out_exe, ZIP_ID)?;
    if back != new_zip {
        return Err(Error::Check("resource 4011 does not read back as written".into()));
    }
    let lines = check_zip(&back, &old_zip, hex_name, hex)?;
    for id in OTHER_RCDATA {
        if res::read(out_exe, id)? != res::read(stock_exe, id)? {
            return Err(Error::Check(format!("resource {id} changed")));
        }
    }
    Ok(lines)
}

/// Reads RT_RCDATA resource `id` of an exe (mapped as data; nothing in it runs).
#[cfg(windows)]
pub fn read_resource(exe: &Path, id: u16) -> Result<Vec<u8>, Error> {
    res::read(exe, id)
}
