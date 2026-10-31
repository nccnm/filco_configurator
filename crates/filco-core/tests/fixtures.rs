//! Real layer pages from the owner's keyboard: every page decodes and re-encodes byte for byte, digests match
//! the manifests, and the difference between consecutive configurations is exactly what their READMEs in the
//! analysis repository describe.

use std::path::PathBuf;

use filco_core::backup;
use filco_core::model::builtin;
use filco_core::{LayerSet, ModelDescriptor, Slot, SlotChange, SlotKind};

const SETS: [&str; 6] = [
    "2026-10-31_0737_original",
    "2026-10-31_LAlt83",
    "2026-10-31_SpaceNav",
    "2026-10-31_Layer1M",
    "2026-10-31_Layer4Apps",
    "2026-10-31_Layer4Snip",
];

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pages")
        .join(name)
}

fn model() -> ModelDescriptor {
    builtin::models().unwrap().remove(0)
}

fn load(name: &str) -> LayerSet {
    let b = backup::read(&dir(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(b.pages.keys().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    LayerSet::new(b.pages.into_values().collect())
}

fn s(text: &str) -> Slot {
    text.parse().unwrap()
}

#[test]
fn every_slot_round_trips_and_is_understood() {
    let m = model();
    let a = m.addressing();
    for name in SETS {
        let set = load(name);
        for (layer, page) in set.pages().iter().enumerate() {
            for index in 0..128u8 {
                let slot = page.slot(a, index).unwrap();
                assert_eq!(slot.kind().to_slot(), slot);
                assert!(
                    !matches!(slot.kind(), SlotKind::Other { .. }),
                    "{name} layer {layer} index {index}: unexpected {slot}"
                );
                if !slot.is_empty() {
                    assert!(
                        m.key_by_index(index).is_some(),
                        "{name}: slot on index {index} has no physical key"
                    );
                }
            }
        }
    }
}

fn diff(a: &str, b: &str) -> Vec<SlotChange> {
    load(a).diff(&load(b), model().addressing())
}

#[test]
fn lalt83_to_spacenav_only_adds_layer_3_navigation() {
    let d = diff("2026-10-31_LAlt83", "2026-10-31_SpaceNav");
    assert!(d.iter().all(|c| c.layer == 2 && c.old.is_empty()), "{d:?}");
    let m = model();
    let at = |id: &str| {
        d.iter()
            .find(|c| c.index == m.key(id).unwrap().index)
            .map(|c| c.new)
    };
    assert_eq!(at("H"), Some(s("02 00 50 00"))); // Left
    assert_eq!(at("J"), Some(s("02 00 51 00"))); // Down
    assert_eq!(at("F"), Some(s("02 01 2a 00"))); // Ctrl + Backspace
    assert_eq!(at("R"), Some(s("01 01 01 00"))); // left click
    assert_eq!(at("N"), Some(s("01 03 ff 00"))); // scroll down
}

#[test]
fn layer4apps_to_layer4snip_changes_only_p_on_layer_4() {
    let d = diff("2026-10-31_Layer4Apps", "2026-10-31_Layer4Snip");
    assert_eq!(
        d,
        vec![SlotChange {
            layer: 3,
            index: 27,
            old: Slot::EMPTY,
            new: s("02 0a 16 00")
        }]
    );
}

#[test]
fn layer1m_to_layer4apps_adds_15_app_hotkeys() {
    let d = diff("2026-10-31_Layer1M", "2026-10-31_Layer4Apps");
    assert_eq!(d.len(), 15);
    assert!(
        d.iter()
            .all(|c| c.layer == 3 && c.new.bytes()[..2] == [0x02, 0x07])
    );
}

#[test]
fn spacenav_to_layer1m_copies_m_keys_of_layer_2_to_layer_1() {
    let set = load("2026-10-31_Layer1M");
    let m = model();
    let a = m.addressing();
    for n in 1..=10 {
        let index = m.key(&format!("M{n}")).unwrap().index;
        assert_eq!(set.slot(a, 0, index), set.slot(a, 1, index), "M{n}");
    }
    let d = diff("2026-10-31_SpaceNav", "2026-10-31_Layer1M");
    assert!(d.iter().all(|c| c.layer == 0));
}

#[test]
fn current_configuration_facts() {
    let set = load("2026-10-31_Layer4Snip");
    let m = model();
    let a = m.addressing();
    for layer in 0..4 {
        assert_eq!(
            set.slot(a, layer, 70),
            Some(s("02 00 a4 00")),
            "M10 = Fn on layer {layer}"
        );
        assert_eq!(
            set.slot(a, layer, 83),
            Some(s("02 00 e2 00")),
            "slot 83 = LAlt on layer {layer}"
        );
    }
    // Layers 1 and 2 are identical (SETUP.md).
    assert_eq!(set.page(0), set.page(1));
}

#[test]
fn original_backup_matches_fn_analysis_section_5() {
    let set = load("2026-10-31_0737_original");
    let a = model().addressing();
    assert_eq!(set.slot(a, 1, 70), Some(s("02 40 e6 00")));
    assert_eq!(set.slot(a, 2, 70), Some(s("02 04 e2 00")));
    assert_eq!(set.slot(a, 0, 70), Some(Slot::EMPTY));
    assert_eq!(set.slot(a, 3, 70), Some(Slot::EMPTY));
}
