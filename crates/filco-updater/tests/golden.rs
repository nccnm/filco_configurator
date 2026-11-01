//! The updater of the golden build HJKL_CapsSpace2_Info (built by `isp_exe.py`, same layout as the updaters
//! flashed successfully) against one built here from the same HEX.
//!
//! The ZIP writer is shown to be Python's: reading the golden archive and writing it back gives the same bytes.
//! The archive built here then equals the golden one entry for entry (name, method, date, CRC, size, data) and
//! byte for byte for the HEX and UISettings.ini; untouched entries carry the stock compressed bytes, where
//! `isp_exe.py` had recompressed them with the deflate of its Python build (zlib-ng), which only changes how the
//! same data is compressed.

#![cfg(windows)]

use std::path::{Path, PathBuf};

use filco_updater::{OTHER_RCDATA, ZIP_ID, read_resource, zip};

const BUILD: &str = "HJKL_CapsSpace2_Info";
const INI_KEYS: [&str; 4] = ["ISPTitle=", "FWName=", "VID=", "PID="];
const HEX: &str = "CT_F72US[0x1321]_20240125_0v72_HJKL_CapsSpace2_Info_PID1972.hex";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn python_zip_layout_is_reproduced() {
    let golden = read_resource(
        &root()
            .join("tests/fixtures/firmware")
            .join(BUILD)
            .join("updater.exe"),
        ZIP_ID,
    )
    .unwrap();
    assert_eq!(zip::write(&zip::read(&golden).unwrap()).unwrap(), golden);
}

#[test]
fn updater_matches_the_golden_one() {
    let dir = root().join("tests/fixtures/firmware").join(BUILD);
    let stock = root().join("firmware").join("M10SP_72US_V72_stock.exe");
    let hex = std::fs::read(dir.join(HEX)).unwrap();
    let out_dir = std::env::temp_dir().join(format!("filco-updater-test-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("updater.exe");
    let lines = filco_updater::build(&stock, HEX, &hex, &out, &format!("_{BUILD}")).unwrap();

    let report = String::from_utf8(std::fs::read(dir.join("TEST_REPORT.txt")).unwrap()).unwrap();
    let want: Vec<&str> = report
        .split("\r\n")
        .filter_map(|l| l.strip_prefix("  "))
        .filter(|l| INI_KEYS.iter().any(|k| l.starts_with(k)))
        .collect();
    assert_eq!(lines, want, "INI lines of the report");

    let golden_exe = dir.join("updater.exe");
    let ours = zip::read(&read_resource(&out, ZIP_ID).unwrap()).unwrap();
    let golden = zip::read(&read_resource(&golden_exe, ZIP_ID).unwrap()).unwrap();
    let stock_entries = zip::read(&read_resource(&stock, ZIP_ID).unwrap()).unwrap();
    assert_eq!(ours.len(), golden.len());
    for (i, (o, g)) in ours.iter().zip(&golden).enumerate() {
        assert_eq!(
            (&o.name, o.method, o.time, o.date, o.crc, o.size),
            (&g.name, g.method, g.time, g.date, g.crc, g.size)
        );
        assert_eq!(o.data().unwrap(), g.data().unwrap(), "{}", o.name);
        if o.name.starts_with("Settings/FWFiles/") || o.name == "Settings/UISettings.ini" {
            assert!(o.raw == g.raw, "{}: compressed bytes differ", o.name);
        } else {
            assert!(
                o.raw == stock_entries[i].raw,
                "{}: not the stock compressed bytes",
                o.name
            );
        }
    }
    for id in OTHER_RCDATA {
        assert_eq!(
            read_resource(&out, id).unwrap(),
            read_resource(&golden_exe, id).unwrap(),
            "resource {id}"
        );
    }
    std::fs::remove_dir_all(&out_dir).unwrap();
}
