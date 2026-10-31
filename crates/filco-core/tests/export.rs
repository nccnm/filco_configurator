//! Export / import of `.filco-layers.json` and compatibility of backups with the `fnmap.py` format.

use std::collections::BTreeMap;
use std::path::PathBuf;

use filco_core::backup::{self, BackupDevice, BackupManifest};
use filco_core::export::{self, ImportError};
use filco_core::model::builtin;
use filco_core::{LayerSet, ModelDescriptor};

fn model() -> ModelDescriptor {
    builtin::models().unwrap().remove(0)
}

fn current() -> LayerSet {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pages/2026-10-31_Layer4Snip");
    LayerSet::new(backup::read(&dir).unwrap().pages.into_values().collect())
}

#[test]
fn export_round_trip() {
    let m = model();
    let set = current();
    let file = export::build(&m, &set, "0.1.0", "2026-10-31T23:59:00+07:00");
    let text = export::to_json(&file);
    assert_eq!(export::parse(&text, &m).unwrap(), set);
    let p = file.layers[3].slots.iter().find(|s| s.index == 27).unwrap();
    assert_eq!(p.key.as_deref(), Some("P"));
    assert_eq!(p.bytes, "02 0a 16 00");
    assert_eq!(p.meaning, "Shift + Win + S");
    assert_eq!(file.product_id, "0x1972");
}

#[test]
fn import_rejects_bad_files() {
    let m = model();
    let file = export::build(&m, &current(), "0.1.0", "now");
    let edit = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = serde_json::to_value(&file).unwrap();
        f(&mut v);
        export::parse(&v.to_string(), &m).unwrap_err()
    };
    assert!(matches!(
        edit(&|v| v["format"] = "x".into()),
        ImportError::Format(_)
    ));
    assert!(matches!(
        edit(&|v| v["format_version"] = 9.into()),
        ImportError::Version(9)
    ));
    assert!(matches!(
        edit(&|v| v["model_id"] = "other".into()),
        ImportError::Model { .. }
    ));
    assert!(matches!(
        edit(&|v| {
            v["layers"].as_array_mut().unwrap().pop();
        }),
        ImportError::LayerCount { got: 3, want: 4 }
    ));
    assert!(matches!(
        edit(&|v| v["layers"][1]["hex"] = "00".into()),
        ImportError::Layer(1, _)
    ));
    let flip = |v: &mut serde_json::Value| {
        let mut h = v["layers"][2]["hex"].as_str().unwrap().to_owned();
        h.replace_range(0..2, if h.starts_with("00") { "01" } else { "00" });
        v["layers"][2]["hex"] = h.into();
    };
    match edit(&flip) {
        ImportError::Layer(2, msg) => assert!(msg.contains("SHA-256")),
        e => panic!("{e:?}"),
    }
    assert!(matches!(export::parse("[", &m), Err(ImportError::Json(_))));
}

#[test]
fn backup_manifest_is_fnmap_compatible() {
    // fnmap.py restore compares manifest["device"]["pid"] with f"{pid:#06x}" and reads manifest["layers"].
    let dir = tempfile::tempdir().unwrap();
    let pages: BTreeMap<usize, _> = current().pages().iter().cloned().enumerate().collect();
    let manifest = BackupManifest {
        created: "20261031-235959".into(),
        device: BackupDevice::new(0x2efd, 0x1972, 0x72, "Majestouch Xacro M10SP US"),
        layers: BTreeMap::new(),
        app: None,
    };
    let out = backup::write(dir.path(), "x", manifest, &pages).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(v["device"]["pid"], "0x1972");
    assert_eq!(v["device"]["rev"], "0x0072");
    assert_eq!(v["layers"]["3"].as_str().unwrap().len(), 64);
    assert!(v.get("app").is_none());
}
