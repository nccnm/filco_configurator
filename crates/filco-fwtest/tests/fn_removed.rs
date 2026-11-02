//! Configs the scenarios once judged wrongly although the firmware was right: a tap-hold key whose stock Fn
//! function the same config removes (its stock keymap byte is a Fn entry reference, not a code), a tap other
//! than the key's own code (Fn + key passes through to the key, not to the tap), a tap-hold key that is one of
//! the keys a scenario presses next to it (J, C, H, D...), and a removed Fn function that let the mode armed by
//! Fn + Left Shift reach the next key of the walk over every key.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn passes(cfg: &str) {
    let stock = String::from_utf8(
        std::fs::read(root().join("firmware").join(filco_firmware::STOCK_HEX_NAME)).unwrap(),
    )
    .unwrap();
    let b = filco_firmware::build(&stock, cfg).unwrap();
    let run = filco_fwtest::run(&b).unwrap();
    let failed: Vec<_> = run.lines.iter().filter(|l| l.ends_with("FAIL")).collect();
    assert!(run.passed, "{cfg}\n{failed:#?}");
}

#[test]
fn key_hold_on_a_key_whose_fn_function_is_removed() {
    passes(
        "name = \"x\"\n[fn_layer]\nSemicolon = \"none\"\n\
         [[tap_hold]]\nkey = \"Semicolon\"\ntap = \"Semicolon\"\nhold = \"RWin\"\n",
    );
}

#[test]
fn key_hold_with_a_tap_other_than_the_key() {
    passes(
        "name = \"x\"\n[fn_layer]\nSemicolon = \"none\"\n\
         [[tap_hold]]\nkey = \"Semicolon\"\ntap = \"Esc\"\nhold = \"RWin\"\n",
    );
}

#[test]
fn layer_hold_with_a_tap_other_than_the_key() {
    passes(
        "name = \"x\"\n[fn_layer]\nSemicolon = \"none\"\n\
         [[tap_hold]]\nkey = \"Semicolon\"\ntap = \"Esc\"\nhold_layer = 3\n",
    );
    passes("name = \"x\"\n[[tap_hold]]\nkey = \"Space#77\"\ntap = \"Esc\"\nhold_layer = 3\n");
}

#[test]
fn tap_hold_on_a_key_the_scenarios_press_next_to_it() {
    passes(
        "name = \"x\"\n[fn_layer]\nJ = \"none\"\n[[tap_hold]]\nkey = \"J\"\ntap = \"J\"\nhold = \"LCtrl\"\n",
    );
    passes("name = \"x\"\n[[tap_hold]]\nkey = \"H\"\ntap = \"H\"\nhold_layer = 2\n");
    passes("name = \"x\"\n[[tap_hold]]\nkey = \"B\"\ntap = \"B\"\nhold_layer = 2\n");
}

/// Fn + Left Shift arms "clear macro" (Fn + Left Shift + 1-4, manual p. 10) until the next key with a Fn function.
/// In key order M follows Left Shift and ended the mode; without its Fn function, 1 came next and was taken as
/// the command. The firmware does what stock does; the walk over every key must not carry the mode along.
#[test]
fn removing_the_fn_function_of_the_key_after_left_shift() {
    passes("name = \"x\"\n[fn_layer]\nM = \"none\"\n");
}
