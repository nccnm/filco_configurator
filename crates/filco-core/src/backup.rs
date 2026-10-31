//! Backup folders in the format of `fnmap.py` (analysis repository): `layer{N}.bin` plus `manifest.json` with
//! the SHA-256 of every layer. `fnmap.py restore` reads backups written here and this module reads theirs.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::page::{PAGE_LEN, Page, PageLenError};

/// `manifest["device"]` of `fnmap.py`. Ids are written `"0x1972"` (lower-case, 4 digits), as `fnmap.py`
/// compares them as text when restoring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDevice {
    pub vid: String,
    pub pid: String,
    pub rev: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl BackupDevice {
    pub fn new(vid: u16, pid: u16, rev: u16, model: &str) -> Self {
        BackupDevice {
            vid: format!("{vid:#06x}"),
            pid: format!("{pid:#06x}"),
            rev: format!("{rev:#06x}"),
            model: model.to_owned(),
            path: None,
        }
    }

    pub fn product_id(&self) -> Option<u16> {
        crate::hex::parse_u16(&self.pid).ok()
    }
}

/// Extra information this application adds; `fnmap.py` ignores unknown keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupAppInfo {
    pub app_version: String,
    pub model_id: String,
    /// Why the backup was made, e.g. "before-write".
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub created: String,
    pub device: BackupDevice,
    /// Layer number (as text) -> lower-case hex SHA-256 of `layer{N}.bin`.
    pub layers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<BackupAppInfo>,
}

/// A backup read from disk, with every layer's digest checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backup {
    pub manifest: BackupManifest,
    /// Layer number -> page.
    pub pages: BTreeMap<usize, Page>,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{path}: manifest is not valid: {source}")]
    Manifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("layer {layer}: {source}")]
    Length { layer: String, source: PageLenError },
    #[error("layer {layer}: SHA-256 does not match the manifest (file damaged or edited)")]
    Digest { layer: String },
    #[error("layer key {0:?} in the manifest is not a layer number")]
    LayerKey(String),
    #[error("{0} already exists")]
    Exists(PathBuf),
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> BackupError + '_ {
    move |source| BackupError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Reads and verifies a backup folder.
pub fn read(dir: &Path) -> Result<Backup, BackupError> {
    let mpath = dir.join("manifest.json");
    let text = fs::read_to_string(&mpath).map_err(io(&mpath))?;
    let manifest: BackupManifest = serde_json::from_str(&text).map_err(|source| BackupError::Manifest {
        path: mpath.clone(),
        source,
    })?;
    let mut pages = BTreeMap::new();
    for (key, digest) in &manifest.layers {
        let layer: usize = key.parse().map_err(|_| BackupError::LayerKey(key.clone()))?;
        let p = dir.join(format!("layer{layer}.bin"));
        let bytes = fs::read(&p).map_err(io(&p))?;
        let page = Page::from_bytes(&bytes).map_err(|source| BackupError::Length {
            layer: key.clone(),
            source,
        })?;
        if !page.sha256_hex().eq_ignore_ascii_case(digest) {
            return Err(BackupError::Digest { layer: key.clone() });
        }
        pages.insert(layer, page);
    }
    Ok(Backup { manifest, pages })
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), BackupError> {
    let mut f = fs::File::create(path).map_err(io(path))?;
    f.write_all(bytes).map_err(io(path))?;
    f.sync_all().map_err(io(path))
}

/// Writes a backup to `root/name` atomically: files go to a temporary folder first, are flushed to disk, then
/// the folder is renamed. Returns the final folder. The manifest's `layers` is filled in from `pages`.
pub fn write(
    root: &Path,
    name: &str,
    mut manifest: BackupManifest,
    pages: &BTreeMap<usize, Page>,
) -> Result<PathBuf, BackupError> {
    fs::create_dir_all(root).map_err(io(root))?;
    let dest = root.join(name);
    if dest.exists() {
        return Err(BackupError::Exists(dest));
    }
    let tmp = root.join(format!(".{name}.partial"));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).map_err(io(&tmp))?;
    }
    fs::create_dir(&tmp).map_err(io(&tmp))?;
    manifest.layers = pages
        .iter()
        .map(|(l, p)| (l.to_string(), p.sha256_hex()))
        .collect();
    for (layer, page) in pages {
        write_synced(&tmp.join(format!("layer{layer}.bin")), page.as_bytes())?;
    }
    let mut text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    text.push('\n');
    write_synced(&tmp.join("manifest.json"), text.as_bytes())?;
    fs::rename(&tmp, &dest).map_err(io(&dest))?;
    // Read it back: a backup that cannot be read must never be relied on.
    let back = read(&dest)?;
    if back.pages != *pages {
        return Err(BackupError::Digest { layer: "*".into() });
    }
    Ok(dest)
}

const _: () = assert!(PAGE_LEN == 512);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressing::SlotAddressing;

    fn sample() -> BTreeMap<usize, Page> {
        let mut pages = BTreeMap::new();
        for l in 0..4 {
            let mut p = Page::empty();
            p.set_slot(
                SlotAddressing::CostarInterleaved8,
                70,
                crate::Slot([2, 0, 0xa4, u8::try_from(l).unwrap()]),
            );
            pages.insert(l, p);
        }
        pages
    }

    fn manifest() -> BackupManifest {
        BackupManifest {
            created: "20261031-235959".into(),
            device: BackupDevice::new(0x2efd, 0x1972, 0x0072, "Majestouch Xacro M10SP US"),
            layers: BTreeMap::new(),
            app: Some(BackupAppInfo {
                app_version: "0.1.0".into(),
                model_id: "filco-m10sp-us".into(),
                reason: "test".into(),
            }),
        }
    }

    #[test]
    fn write_then_read() {
        let dir = tempfile::tempdir().unwrap();
        let pages = sample();
        let out = write(dir.path(), "b1", manifest(), &pages).unwrap();
        let back = read(&out).unwrap();
        assert_eq!(back.pages, pages);
        assert_eq!(back.manifest.device.pid, "0x1972");
        assert!(matches!(
            write(dir.path(), "b1", manifest(), &pages),
            Err(BackupError::Exists(_))
        ));
        assert!(!dir.path().join(".b1.partial").exists());
    }

    #[test]
    fn detects_damage() {
        let dir = tempfile::tempdir().unwrap();
        let out = write(dir.path(), "b1", manifest(), &sample()).unwrap();
        let p = out.join("layer2.bin");
        let mut bytes = fs::read(&p).unwrap();
        bytes[0] ^= 1;
        fs::write(&p, &bytes).unwrap();
        assert!(matches!(read(&out), Err(BackupError::Digest { layer }) if layer == "2"));
        fs::write(&p, &bytes[..100]).unwrap();
        assert!(matches!(read(&out), Err(BackupError::Length { .. })));
        fs::remove_file(&p).unwrap();
        assert!(matches!(read(&out), Err(BackupError::Io { .. })));
        fs::write(out.join("manifest.json"), "{").unwrap();
        assert!(matches!(read(&out), Err(BackupError::Manifest { .. })));
    }
}
