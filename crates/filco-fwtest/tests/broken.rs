//! The tests must catch a wrong image: a Fn table byte changed behind the build's back, and a tap-hold
//! expectation the code does not meet.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn build(name: &str) -> filco_firmware::Build {
    let read = |p: PathBuf| String::from_utf8(std::fs::read(&p).unwrap()).unwrap();
    let stock = read(root().join("firmware").join(filco_firmware::STOCK_HEX_NAME));
    let cfg = read(
        root()
            .join("tests/fixtures/firmware")
            .join(name)
            .join("config.toml"),
    );
    filco_firmware::build(&stock, &cfg).unwrap()
}

#[test]
fn a_changed_fn_table_byte_fails() {
    let mut b = build("HJKL");
    b.image[0xC6E7] = 0x51; // QWERTY Fn entry 34: Fn + H = Down instead of Left
    let run = filco_fwtest::run(&b).unwrap();
    assert!(!run.passed);
    assert!(
        run.lines
            .iter()
            .any(|l| l.starts_with("  Fn + H") && l.ends_with("FAIL")),
        "{:#?}",
        run.lines
    );
}

#[test]
fn a_wrong_tap_hold_expectation_fails() {
    let mut b = build("HJKL_CapsTapHold");
    let (k, _, hold, term) = b.config.tap_key.unwrap();
    b.config.tap_key = Some((k, 0x2B, hold, term)); // expect Tab where the code sends Esc
    let run = filco_fwtest::run(&b).unwrap();
    assert!(!run.passed);
    assert!(
        run.lines
            .iter()
            .any(|l| l.contains("tapped 40 ms") && l.ends_with("FAIL"))
    );
}
