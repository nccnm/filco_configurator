//! Every golden build of the analysis repository (`tests/fixtures/firmware/`) must come out byte for byte the
//! same: image, HEX (name and content, which carry the checksum tag) and the patch log of PATCH_DIFF.txt (the
//! first two lines hold machine paths and are skipped).

use std::path::{Path, PathBuf};

const BUILDS: [&str; 5] = [
    "HJKL",
    "HJKL_CapsTapHold",
    "HJKL_CapsSpace",
    "HJKL_CapsSpace2",
    "HJKL_CapsSpace2_Info",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn stock_hex() -> String {
    String::from_utf8(read(
        &root().join("firmware").join(filco_firmware::STOCK_HEX_NAME),
    ))
    .unwrap()
}

#[test]
fn golden_builds_are_reproduced_byte_for_byte() {
    let stock = stock_hex();
    for name in BUILDS {
        let dir = root().join("tests").join("fixtures").join("firmware").join(name);
        let cfg = String::from_utf8(read(&dir.join("config.toml"))).unwrap();
        let b = filco_firmware::build(&stock, &cfg).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(b.image == read(&dir.join("image.bin")), "{name}: image differs");
        let gold_hex = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|x| x == "hex"))
            .unwrap();
        assert_eq!(
            gold_hex.file_name().to_string_lossy(),
            b.hex_name,
            "{name}: HEX name (tag)"
        );
        assert!(
            b.hex_text.as_bytes() == read(&gold_hex.path()),
            "{name}: HEX content differs"
        );
        let gold_diff = String::from_utf8(read(&dir.join("PATCH_DIFF.txt"))).unwrap();
        let ours = filco_firmware::crlf_text(&b.patch_diff_lines("", ""));
        let skip2 = |s: &str| s.splitn(3, "\r\n").nth(2).unwrap_or("").to_string();
        assert_eq!(skip2(&ours), skip2(&gold_diff), "{name}: PATCH_DIFF differs");
        assert_eq!(
            gold_diff
                .lines()
                .nth(1)
                .map(|l| l.ends_with("(word sum F3EA, tag F448)")),
            Some(true)
        );
    }
}

#[test]
fn refuses_a_non_stock_hex_and_bad_configs() {
    let stock = stock_hex();
    let mut other = stock.clone();
    other.push_str("\r\n");
    assert!(matches!(
        filco_firmware::build(&other, "name = \"x\"\ninfo_block = true\n"),
        Err(filco_firmware::Error::NotStockHex)
    ));
    let bad = [
        "name = \"x\"\n",
        "name = \"bad name\"\ninfo_block = true\n",
        "name = \"x\"\nfoo = 1\n",
        "name = \"x\"\n[fn_layer]\nNoSuchKey = \"Left\"\n",
        "name = \"x\"\n[keymap]\nH = \"none\"\n",
        "name = \"x\"\n[[tap_hold]]\nkey = \"CapsLock\"\ntap = \"Esc\"\n",
        "name = \"x\"\n[[tap_hold]]\nkey = \"CapsLock\"\ntap = \"Esc\"\nhold = \"LCtrl\"\nterm_ms = 300\n",
        "name = \"x\"\n[[tap_hold]]\nkey = \"Space#77\"\ntap = \"Space\"\nhold_layer = 5\n",
        "name = \"x\"\n[[tap_hold]]\nkey = \"I\"\ntap = \"Esc\"\nhold = \"LCtrl\"\n",
        "name = \"x\"\n[[tap_hold]]\nkey = \"Fn\"\ntap = \"Esc\"\nhold = \"LCtrl\"\n",
        "name = \"x\"\n[keymap]\nH = \"Fn\"\n",
    ];
    for cfg in bad {
        assert!(filco_firmware::build(&stock, cfg).is_err(), "accepted: {cfg}");
    }
}
