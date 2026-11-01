//! The firmware build pipeline (port of `build.py` `main()` in the analysis repository). All offline: nothing
//! is sent to a keyboard and the updater is never run.
//!
//! 1. Read the stock V72 HEX from the stock folder and check its SHA-256.
//! 2. Apply the config ([`filco_firmware::build`]).
//! 3. Write `<out>/<name>/`: the HEX (its file name carries the checksum tag), `image.bin`, `PATCH_DIFF.txt`.
//! 4. Run the image and stock side by side in the emulator ([`filco_fwtest::run`]).
//! 5. Only when every test passes, make `<out>/<name>/M10SP_72US_V72_<name>.exe` from the stock updater.
//!
//! `TEST_REPORT.txt` holds the whole report either way. Flashing is the user's: run that exe, check that it
//! shows `CheckSum : 0x<tag>`, press Start.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use filco_firmware::v72::{STOCK_UPDATER_SHA256, sha256_hex};
use filco_firmware::{STOCK_HEX_NAME, STOCK_UPDATER_NAME, crlf_text, word_sum};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Firmware(#[from] filco_firmware::Error),
    #[error(transparent)]
    Updater(#[from] filco_updater::Error),
    #[error("{path}: {msg}")]
    Io { path: PathBuf, msg: String },
    #[error("stock updater sha256 mismatch")]
    NotStockUpdater,
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::Io {
        path: path.to_path_buf(),
        msg: e.to_string(),
    }
}

/// What a build produced.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub name: String,
    /// Checksum tag the updater must show.
    pub tag: u16,
    pub dir: PathBuf,
    pub hex: PathBuf,
    /// The updater, only when every test passed.
    pub updater: Option<PathBuf>,
    pub passed: bool,
    pub report: Vec<String>,
}

/// Runs the pipeline. `stock_dir` holds the stock HEX and updater; results go to `out_root/<name>/`, whose
/// earlier files are removed first. `progress` gets each report line as it is written.
pub fn run(
    stock_dir: &Path,
    config_path: &Path,
    out_root: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<Outcome, Error> {
    let stock_hex_path = stock_dir.join(STOCK_HEX_NAME);
    let stock_hex = std::fs::read(&stock_hex_path).map_err(io(&stock_hex_path))?;
    let stock_hex = String::from_utf8(stock_hex).map_err(|_| filco_firmware::Error::NotStockHex)?;
    let config = std::fs::read(config_path).map_err(io(config_path))?;
    let config =
        String::from_utf8(config).map_err(|_| filco_firmware::Error::Config("config is not UTF-8".into()))?;
    let b = filco_firmware::build(&stock_hex, &config)?;
    let name = b.config.name.clone();
    let tag = b.tag();

    let dir = out_root.join(&name);
    std::fs::create_dir_all(&dir).map_err(io(&dir))?;
    for e in std::fs::read_dir(&dir).map_err(io(&dir))?.flatten() {
        if e.file_type().is_ok_and(|t| t.is_file()) {
            std::fs::remove_file(e.path()).map_err(io(&e.path()))?;
        }
    }
    let hex = dir.join(&b.hex_name);
    std::fs::write(&hex, b.hex_text.as_bytes()).map_err(io(&hex))?;
    let bin = dir.join("image.bin");
    std::fs::write(&bin, &b.image).map_err(io(&bin))?;

    let mut report = b.patch_diff_lines(
        &config_path.display().to_string(),
        &stock_hex_path.display().to_string(),
    );
    for l in &report {
        progress(l);
    }
    let diff = dir.join("PATCH_DIFF.txt");
    std::fs::write(&diff, crlf_text(&report)).map_err(io(&diff))?;
    emit(progress, &mut report, String::new());
    // Test lines reach `progress` as each test runs; they join the report once the run is over.
    let passed = match filco_fwtest::run_streaming(&b, &mut |l| progress(l)) {
        Ok(t) => {
            report.extend(t.lines);
            t.passed
        }
        Err(e) => {
            emit(progress, &mut report, format!("  emulator stopped: {e}"));
            false
        }
    };
    emit(progress, &mut report, String::new());
    let test_report = dir.join("TEST_REPORT.txt");
    let mut updater = None;
    if passed {
        let stock_exe = stock_dir.join(STOCK_UPDATER_NAME);
        if sha256_hex(&std::fs::read(&stock_exe).map_err(io(&stock_exe))?) != STOCK_UPDATER_SHA256 {
            return Err(Error::NotStockUpdater);
        }
        let exe = dir.join(format!("M10SP_72US_V72_{name}.exe"));
        for l in filco_updater::build(
            &stock_exe,
            &b.hex_name,
            b.hex_text.as_bytes(),
            &exe,
            &format!("_{name}"),
        )? {
            emit(progress, &mut report, format!("  {l}"));
        }
        emit(
            progress,
            &mut report,
            format!("All tests passed. Updater: {}", exe.display()),
        );
        emit(
            progress,
            &mut report,
            format!("Before pressing Start, the updater must show CheckSum : 0x{tag:04X}"),
        );
        updater = Some(exe);
    } else {
        emit(
            progress,
            &mut report,
            "TESTS FAILED: no updater built. Fix the config or the tool before flashing anything.".into(),
        );
    }
    std::fs::write(&test_report, crlf_text(&report)).map_err(io(&test_report))?;
    debug_assert_eq!(
        word_sum(&b.image).wrapping_add(filco_firmware::v72::TAG_OFFSET),
        tag
    );
    Ok(Outcome {
        name,
        tag,
        dir,
        hex,
        updater,
        passed,
        report,
    })
}

/// Hands a report line to `progress` and keeps it for the report.
fn emit(progress: &mut dyn FnMut(&str), report: &mut Vec<String>, s: String) {
    progress(&s);
    report.push(s);
}
