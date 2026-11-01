//! The tests of every golden build must report line for line what the analysis repository's Unicorn run wrote
//! to TEST_REPORT.txt (the part between the patch log and the updater lines).

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

fn text(p: &Path) -> String {
    String::from_utf8(std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))).unwrap()
}

fn check(name: &str) {
    let stock = text(&root().join("firmware").join(filco_firmware::STOCK_HEX_NAME));
    let dir = root().join("tests").join("fixtures").join("firmware").join(name);
    let b = filco_firmware::build(&stock, &text(&dir.join("config.toml"))).unwrap();
    let t0 = std::time::Instant::now();
    let run = filco_fwtest::run(&b).unwrap_or_else(|e| panic!("{name}: emulator stopped: {e}"));
    eprintln!("{name}: tests ran in {:.1} s", t0.elapsed().as_secs_f64());
    let report = text(&dir.join("TEST_REPORT.txt"));
    let lines: Vec<&str> = report.split("\r\n").collect();
    let start = b.patch_diff_lines("", "").len() + 1;
    let end = start + lines[start..].iter().position(|l| l.is_empty()).unwrap();
    let want = &lines[start..end];
    for (i, (g, w)) in run.lines.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{name}: report line {} differs", start + i + 1);
    }
    assert_eq!(run.lines.len(), want.len(), "{name}: report length");
    assert!(run.passed, "{name}: tests failed");
}

#[test]
fn hjkl() {
    check(BUILDS[0]);
}

#[test]
fn hjkl_caps_tap_hold() {
    check(BUILDS[1]);
}

#[test]
fn hjkl_caps_space() {
    check(BUILDS[2]);
}

#[test]
fn hjkl_caps_space2() {
    check(BUILDS[3]);
}

#[test]
fn hjkl_caps_space2_info() {
    check(BUILDS[4]);
}
