//! Decode the info block out of the golden firmware images built by the analysis repository, and check the
//! builds without a block are recognised as having none.

use std::path::PathBuf;

use filco_fwinfo::{ReadError, from_image, parse};

const ESC: u8 = 0x29;
const LCTRL: u8 = 0xE0;
const SPACE: u8 = 0x2C;
const LEFT: u8 = 0x50;
const RIGHT: u8 = 0x4F;
const UP: u8 = 0x52;
const DOWN: u8 = 0x51;

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "fixtures",
        "firmware",
        name,
        "image.bin",
    ]
    .iter()
    .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn reads_the_capsspace2_info_block() {
    let info = from_image(&fixture("HJKL_CapsSpace2_Info")).expect("this build has an info block");

    assert_eq!(info.name, "HJKL_CapsSpace2_Info");
    assert_eq!(info.format, 1);
    assert_eq!(info.base_version, 0x0072);
    assert_eq!(info.stock_tag, 0xF448);

    // CapsLock: tap Esc, hold LCtrl, 200 ms.
    let caps = info.tap_key.expect("CapsLock is a key-hold tap-hold key");
    assert_eq!((caps.tap, caps.hold, caps.term_ms), (ESC, LCTRL, 200));

    // Two Space keys hold a layer: #77 -> layer index 2 (LED 3), #76 -> layer index 3 (LED 4), both 300 ms.
    assert_eq!(info.tap_layers.len(), 2);
    for tl in &info.tap_layers {
        assert_eq!(tl.tap, SPACE);
        assert_eq!(tl.term_ms, 300);
    }
    let by_key: std::collections::BTreeMap<u8, u8> =
        info.tap_layers.iter().map(|t| (t.key, t.layer)).collect();
    assert_eq!(by_key, [(76, 3), (77, 2)].into_iter().collect());

    // Fn layer: H/J/K/L remapped to arrows, I's Fn function removed.
    let mut codes: Vec<Option<u8>> = info.fn_layer.iter().map(|f| f.code).collect();
    codes.sort();
    let mut want = vec![Some(LEFT), Some(DOWN), Some(UP), Some(RIGHT), None];
    want.sort();
    assert_eq!(codes, want);

    // This build does not change any default keymap code.
    assert_eq!(info.keymap, []);
}

#[test]
fn block_round_trips_through_serde() {
    let info = from_image(&fixture("HJKL_CapsSpace2_Info")).unwrap();
    let json = serde_json::to_string(&info).unwrap();
    assert_eq!(serde_json::from_str::<filco_fwinfo::Info>(&json).unwrap(), info);
}

#[test]
fn builds_without_a_block_report_no_block() {
    for name in ["HJKL", "HJKL_CapsTapHold", "HJKL_CapsSpace", "HJKL_CapsSpace2"] {
        assert_eq!(
            from_image(&fixture(name)),
            Err(ReadError::NoBlock),
            "{name} has no info block"
        );
    }
}

#[test]
fn empty_and_short_input_is_no_block() {
    assert_eq!(parse(&[]), Err(ReadError::NoBlock));
    assert_eq!(parse(b"KLE"), Err(ReadError::NoBlock));
    assert_eq!(parse(b"NOPE............"), Err(ReadError::NoBlock));
}
