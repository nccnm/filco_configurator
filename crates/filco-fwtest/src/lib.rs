//! Test a built V72 image in the [`m0emu`] interpreter, side by side with stock (port of the test part of
//! `build.py` in the analysis repository).
//!
//! [`run`] boots the stock and the new image and checks every key plain and with Fn, the Fn key, layer
//! switching, the Fn layer through the M10 user slot, every tap-hold key with timed scenarios (one emulator step
//! is one 1 ms tick), the info command over an emulated HID host and the static worst-case stack depth of every
//! entry point. The report lines are those `build.py` writes to TEST_REPORT.txt, so a run on a golden build
//! compares line for line with the report Unicorn produced.
#![forbid(unsafe_code)]

pub mod hid;
pub mod machine;
pub mod scenarios;
pub mod stack;

use filco_firmware::Build;
pub use m0emu::Stop;

/// Result of a test run.
#[derive(Debug, Clone)]
pub struct TestRun {
    pub passed: bool,
    pub lines: Vec<String>,
}

/// Runs every scenario on `build`. An `Err` means the emulator stopped (unsupported instruction, a call that
/// did not return): the image is not proven and must not be flashed.
pub fn run(build: &Build) -> Result<TestRun, Stop> {
    let mut out = scenarios::Out::default();
    let passed = scenarios::run_all(build, &mut out)?;
    Ok(TestRun {
        passed,
        lines: out.lines,
    })
}
