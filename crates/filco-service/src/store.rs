//! The backup folder: `<root>/<model id>/<YYYYMMDD-HHMMSS>/` in the `fnmap.py` format.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use filco_core::backup::{self, BackupAppInfo, BackupDevice, BackupError, BackupManifest};
use filco_core::{LayerSet, ModelDescriptor, Page};
use serde::Serialize;

use crate::discovery::InterfaceInfo;

/// Local time as used in backup names and manifests, e.g. `20261101-021500`.
pub fn timestamp_now() -> String {
    jiff::Zoned::now().strftime("%Y%m%d-%H%M%S").to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupEntry {
    pub name: String,
    pub path: PathBuf,
    pub created: String,
    /// Why it was made ("before-write", "manual", ...); `None` for backups made by `fnmap.py`.
    pub reason: Option<String>,
    /// Set when the folder cannot be used (missing layer, wrong digest...).
    pub problem: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupStore {
    root: PathBuf,
}

impl BackupStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        BackupStore { root: root.into() }
    }

    /// `%LOCALAPPDATA%\FilcoConfigurator\backups`, or `None` without `LOCALAPPDATA`.
    pub fn default_root() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("FilcoConfigurator").join("backups"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn model_dir(&self, model: &ModelDescriptor) -> PathBuf {
        self.root.join(&model.id)
    }

    /// Saves all layers. `created` is the timestamp (see [`timestamp_now`]); a suffix is added when a backup of
    /// the same second exists.
    pub fn save(
        &self,
        model: &ModelDescriptor,
        device: &InterfaceInfo,
        pages: &LayerSet,
        reason: &str,
        created: &str,
    ) -> Result<PathBuf, BackupError> {
        let dir = self.model_dir(model);
        let mut name = created.to_owned();
        let mut n = 2;
        while dir.join(&name).exists() {
            name = format!("{created}-{n}");
            n += 1;
        }
        let manifest = BackupManifest {
            created: created.to_owned(),
            device: BackupDevice::new(device.vendor_id, device.product_id, device.version, &model.name),
            layers: BTreeMap::new(),
            app: Some(BackupAppInfo {
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                model_id: model.id.clone(),
                reason: reason.to_owned(),
            }),
        };
        let map: BTreeMap<usize, Page> = pages.pages().iter().cloned().enumerate().collect();
        backup::write(&dir, &name, manifest, &map)
    }

    /// Backups of `model`, newest first. Folders that fail verification are listed with their problem.
    pub fn list(&self, model: &ModelDescriptor) -> Vec<BackupEntry> {
        let dir = self.model_dir(model);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out: Vec<BackupEntry> = rd
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                match load(&path, model) {
                    Ok((manifest, _)) => BackupEntry {
                        name,
                        path,
                        created: manifest.created,
                        reason: manifest.app.map(|a| a.reason),
                        problem: None,
                    },
                    Err(err) => BackupEntry {
                        created: name.clone(),
                        name,
                        path,
                        reason: None,
                        problem: Some(err.to_string()),
                    },
                }
            })
            .collect();
        out.sort_by(|a, b| b.name.cmp(&a.name));
        out
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Backup(#[from] BackupError),
    #[error("backup is for {found}, not {expected}")]
    WrongDevice { found: String, expected: String },
    #[error("backup has layers {found:?}; this model needs 0..{expected}")]
    Layers { found: Vec<usize>, expected: usize },
}

/// Reads a backup folder (ours or `fnmap.py`'s) and checks it holds every layer of `model`.
pub fn load(dir: &Path, model: &ModelDescriptor) -> Result<(BackupManifest, LayerSet), LoadError> {
    let b = backup::read(dir)?;
    if let Some(pid) = b.manifest.device.product_id()
        && !model.matches(model.device.vendor_id.0, pid)
    {
        return Err(LoadError::WrongDevice {
            found: b.manifest.device.pid.clone(),
            expected: model.name.clone(),
        });
    }
    let n = model.layers.len();
    let keys: Vec<usize> = b.pages.keys().copied().collect();
    if keys != (0..n).collect::<Vec<_>>() {
        return Err(LoadError::Layers {
            found: keys,
            expected: n,
        });
    }
    Ok((b.manifest, LayerSet::new(b.pages.into_values().collect())))
}

#[cfg(test)]
mod tests {
    use filco_core::model::builtin;

    use super::*;

    fn info() -> InterfaceInfo {
        InterfaceInfo {
            path: "sim".into(),
            vendor_id: 0x2efd,
            product_id: 0x1972,
            version: 0x72,
            usage_page: 1,
            input_len: 9,
            output_len: 2,
            feature_len: 65,
        }
    }

    #[test]
    fn save_list_load() {
        let m = builtin::models().unwrap().remove(0);
        let tmp = tempfile::tempdir().unwrap();
        let store = BackupStore::new(tmp.path());
        let mut pages = LayerSet::empty(4);
        pages
            .page_mut(3)
            .unwrap()
            .set_slot(m.addressing(), 70, "02 00 a4 00".parse().unwrap());
        let a = store
            .save(&m, &info(), &pages, "before-write", "20261101-010000")
            .unwrap();
        let b = store
            .save(&m, &info(), &pages, "manual", "20261101-010000")
            .unwrap();
        assert!(b.ends_with("20261101-010000-2"));
        std::fs::create_dir_all(store.model_dir(&m).join("20261101-020000")).unwrap();
        let list = store.list(&m);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "20261101-020000");
        assert!(list[0].problem.is_some());
        assert_eq!(list[1].reason.as_deref(), Some("manual"));
        let (manifest, loaded) = load(&a, &m).unwrap();
        assert_eq!(loaded, pages);
        assert_eq!(manifest.device.pid, "0x1972");
    }

    #[test]
    fn fnmap_backups_load_and_ours_have_fnmap_shape() {
        let m = builtin::models().unwrap().remove(0);
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pages/2026-10-31_0737_original");
        let (manifest, pages) = load(&fixture, &m).unwrap();
        assert!(manifest.app.is_none(), "written by fnmap.py");
        assert_eq!(pages.len(), 4);

        let tmp = tempfile::tempdir().unwrap();
        let out = BackupStore::new(tmp.path())
            .save(&m, &info(), &pages, "manual", "20261101-030000")
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
        // The keys fnmap.py restore reads.
        assert_eq!(v["device"]["vid"], "0x2efd");
        assert_eq!(v["device"]["pid"], "0x1972");
        for l in 0..4 {
            let digest = v["layers"][l.to_string()].as_str().unwrap();
            assert_eq!(digest, pages.page(l).unwrap().sha256_hex());
            assert_eq!(
                std::fs::read(out.join(format!("layer{l}.bin"))).unwrap().len(),
                512
            );
        }
    }

    #[test]
    fn rejects_other_devices_and_missing_layers() {
        let m = builtin::models().unwrap().remove(0);
        let tmp = tempfile::tempdir().unwrap();
        let store = BackupStore::new(tmp.path());
        let jp = InterfaceInfo {
            product_id: 0x1976,
            ..info()
        };
        let dir = store
            .save(&m, &jp, &LayerSet::empty(4), "manual", "20261101-040000")
            .unwrap();
        assert!(matches!(load(&dir, &m), Err(LoadError::WrongDevice { .. })));
        let dir = store
            .save(&m, &info(), &LayerSet::empty(3), "manual", "20261101-050000")
            .unwrap();
        assert!(matches!(load(&dir, &m), Err(LoadError::Layers { .. })));
    }
}
