//! Device::read_info reads and decodes the firmware info block through the full service + protocol + sim stack.

use std::path::Path;

use filco_core::model::builtin;
use filco_core::{ModelDescriptor, PAGE_LEN, Page};
use filco_service::backend::SimBackend;
use filco_service::device::Device;
use filco_sim::SimKeyboard;

const BLOCK_ADDR: usize = 0xE600;

fn model() -> ModelDescriptor {
    builtin::models().unwrap().remove(0)
}

fn info_block_of(build: &str) -> [u8; PAGE_LEN] {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/firmware")
        .join(build)
        .join("image.bin");
    let image = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    image[BLOCK_ADDR..BLOCK_ADDR + PAGE_LEN]
        .try_into()
        .expect("512-byte block")
}

fn device(sim: SimKeyboard) -> Device {
    let backend = SimBackend::new(sim);
    Device::open(&backend, &backend.info.clone(), &model()).unwrap()
}

#[test]
fn decodes_the_block_from_a_patched_keyboard() {
    let mut dev = device(SimKeyboard::blank(4).with_info_block(info_block_of("HJKL_CapsSpace2_Info")));
    let info = dev
        .read_info(&mut |_| {})
        .unwrap()
        .expect("a patched keyboard has an info block");

    assert_eq!(info.name, "HJKL_CapsSpace2_Info");
    assert_eq!(info.tap_key.expect("CapsLock").tap, 0x29); // Esc
    assert_eq!(info.tap_layers.len(), 2);
    assert_eq!(info.fn_layer.len(), 5); // H/J/K/L remapped, I removed
}

#[test]
fn stock_keyboard_has_no_info() {
    let mut dev = device(SimKeyboard::new(vec![Page::empty(); 4]));
    assert_eq!(dev.read_info(&mut |_| {}).unwrap(), None);
}
