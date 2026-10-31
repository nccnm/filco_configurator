//! filco-protocol + filco-sim reproduce fnmap.py's transcripts byte for byte
//! (tests/fixtures/golden, made by tools/golden/gen_fnmap_golden.py from fnmap.py itself).

use std::path::PathBuf;
use std::time::Duration;

use filco_core::{Page, backup};
use filco_protocol::{Delays, Session, VirtualClock};
use filco_sim::{Direction, SimKeyboard};
use serde::Deserialize;

#[derive(Deserialize)]
struct Golden {
    device_pages: String,
    write: Option<WriteSpec>,
    reports: Vec<Report>,
}

#[derive(Deserialize)]
struct WriteSpec {
    layer: u8,
    page: String,
}

#[derive(Deserialize)]
struct Report {
    dir: String,
    hex: String,
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn pages(name: &str) -> Vec<Page> {
    backup::read(&fixtures().join("pages").join(name))
        .unwrap()
        .pages
        .into_values()
        .collect()
}

fn golden(name: &str) -> Golden {
    let text = std::fs::read_to_string(fixtures().join("golden").join(format!("{name}.json"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn transcript(sim: &SimKeyboard) -> Vec<(String, String)> {
    sim.transcript()
        .iter()
        .map(|(d, r)| {
            let dir = match d {
                Direction::Set => "set",
                Direction::Get => "get",
            };
            (dir.to_owned(), filco_core::hex::encode(r))
        })
        .collect()
}

fn expected(g: &Golden) -> Vec<(String, String)> {
    g.reports.iter().map(|r| (r.dir.clone(), r.hex.clone())).collect()
}

fn assert_same(got: &[(String, String)], want: &[(String, String)]) {
    assert_eq!(got.len(), want.len(), "number of reports");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "report {i} differs from fnmap.py");
    }
}

#[test]
fn read_all_layers_matches_fnmap() {
    let g = golden("read_layer4snip");
    let want_pages = pages(&g.device_pages);
    let mut session = Session::new(
        SimKeyboard::new(want_pages.clone()),
        VirtualClock::default(),
        Delays::FILCO_ASSIST,
    );
    for layer in 0..4u8 {
        let p = session.read_page(layer, &mut |_| {}).unwrap();
        assert_eq!(p, want_pages[usize::from(layer)]);
    }
    assert_same(&transcript(session.transport()), &expected(&g));
    // 12 reports per page, a 100 ms pause after each: 4.8 s of protocol time for 4 layers.
    assert_eq!(session.clock().elapsed, Duration::from_millis(4800));
}

#[test]
fn write_one_layer_with_readback_matches_fnmap() {
    let g = golden("write_layer3_snip_over_apps");
    let w = g.write.as_ref().unwrap();
    let new_page = pages(&w.page)[usize::from(w.layer)].clone();
    let mut session = Session::new(
        SimKeyboard::new(pages(&g.device_pages)),
        VirtualClock::default(),
        Delays::FILCO_ASSIST,
    );
    session.write_page(w.layer, &new_page, &mut |_| {}).unwrap();
    assert_same(&transcript(session.transport()), &expected(&g));
    assert_eq!(session.transport().flash()[usize::from(w.layer)], new_page);
    assert_eq!(session.transport().flash_writes(), 1);
}
