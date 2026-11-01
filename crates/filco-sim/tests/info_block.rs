//! Reading the firmware info block (command 0x30) over filco-protocol + filco-sim: a keyboard with a block
//! returns it as a read session; one without a block NACKs 0x30, which the session reports as "no block".

use std::path::PathBuf;

use filco_core::{PAGE_LEN, Page};
use filco_protocol::{Delays, Session, VirtualClock};
use filco_sim::SimKeyboard;

const BLOCK_ADDR: usize = 0xE600;

fn info_block_of(build: &str) -> [u8; PAGE_LEN] {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "fixtures",
        "firmware",
        build,
        "image.bin",
    ]
    .iter()
    .collect();
    let image = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    image[BLOCK_ADDR..BLOCK_ADDR + PAGE_LEN]
        .try_into()
        .expect("512-byte block")
}

fn session(sim: SimKeyboard) -> Session<SimKeyboard, VirtualClock> {
    Session::new(sim, VirtualClock::default(), Delays::FILCO_ASSIST)
}

#[test]
fn reads_the_block_from_a_patched_keyboard() {
    let block = info_block_of("HJKL_CapsSpace2_Info");
    let mut session = session(SimKeyboard::blank(4).with_info_block(block));

    let got = session.read_info(&mut |_| {}).unwrap();
    assert_eq!(got, Some(block), "0x30 returns the block, checksum verified");
}

#[test]
fn stock_keyboard_reports_no_block() {
    // No info block set: 0x30 is an unknown command and goes unacknowledged, like stock firmware.
    let mut session = session(SimKeyboard::blank(4));
    assert_eq!(session.read_info(&mut |_| {}).unwrap(), None);
}

#[test]
fn reading_layers_still_works_after_an_info_read() {
    let block = info_block_of("HJKL_CapsSpace2_Info");
    let pages: Vec<Page> = (0..4).map(|_| Page::empty()).collect();
    let mut session = session(SimKeyboard::new(pages).with_info_block(block));

    assert_eq!(session.read_info(&mut |_| {}).unwrap(), Some(block));
    for layer in 0..4u8 {
        session
            .read_page(layer, &mut |_| {})
            .expect("layer read after info read");
    }
}
