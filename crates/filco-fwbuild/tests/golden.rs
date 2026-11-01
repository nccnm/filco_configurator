//! The whole pipeline on every golden build: the files written equal the golden ones (image, HEX, PATCH_DIFF
//! and TEST_REPORT, apart from the lines that print machine paths) and an updater exists exactly when all
//! tests passed.

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

fn without_paths(text: &str) -> String {
    text.split("\r\n")
        .filter(|l| {
            !l.starts_with("Config: ")
                && !l.starts_with("Stock:  ")
                && !l.starts_with("All tests passed. Updater: ")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

#[test]
fn pipeline_reproduces_every_golden_build() {
    let out = tempfile::tempdir().unwrap();
    // One thread per build: each is independent and writes only its own out/<name>/.
    std::thread::scope(|s| {
        for name in BUILDS {
            let out = out.path();
            s.spawn(move || check(name, out));
        }
    });
}

fn check(name: &str, out: &Path) {
    let gold = root().join("tests/fixtures/firmware").join(name);
    let mut seen = Vec::new();
    let o = filco_fwbuild::run(
        &root().join("firmware"),
        &gold.join("config.toml"),
        out,
        &mut |l| seen.push(l.to_owned()),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(o.passed, "{name}: tests failed");
    assert_eq!(
        seen, o.report,
        "{name}: progress gets every report line, in order"
    );
    let exe = o.updater.as_ref().expect("updater built");
    assert!(exe.is_file());
    let read = |p: PathBuf| std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    assert!(
        read(o.dir.join("image.bin")) == read(gold.join("image.bin")),
        "{name}: image"
    );
    let hex_name = o.hex.file_name().unwrap().to_owned();
    assert!(read(o.hex.clone()) == read(gold.join(&hex_name)), "{name}: HEX");
    for f in ["PATCH_DIFF.txt", "TEST_REPORT.txt"] {
        let ours = String::from_utf8(read(o.dir.join(f))).unwrap();
        let want = String::from_utf8(read(gold.join(f))).unwrap();
        assert_eq!(without_paths(&ours), without_paths(&want), "{name}: {f}");
    }
}

#[test]
fn a_config_error_writes_nothing() {
    // A config error stops the build before any file is written.
    let out = tempfile::tempdir().unwrap();
    let cfg = out.path().join("bad.toml");
    std::fs::write(&cfg, "name = \"Bad\"\n[fn_layer]\nH = \"NoSuchCode\"\n").unwrap();
    assert!(filco_fwbuild::run(&root().join("firmware"), &cfg, out.path(), &mut |_| {}).is_err());
    assert!(!out.path().join("Bad").exists());
}
