//! Application cost of reading all layers, without HID time or pauses: the simulated keyboard answers at once and
//! the clock does not sleep. Budget (PLAN.md section 8): at most 5 ms for 4 layers.
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use filco_core::model::builtin;
use filco_core::{LayerSet, Page};
use filco_protocol::{Delays, Session, VirtualClock};
use filco_service::editor::Editor;
use filco_sim::SimKeyboard;

fn read_four_layers(c: &mut Criterion) {
    let model = builtin::models().unwrap().remove(0);
    c.bench_function("read 4 layers (sim, no pauses)", |b| {
        let mut session = Session::new(
            SimKeyboard::new(vec![Page::empty(); 4]),
            VirtualClock::default(),
            Delays::from_ms(0, 0),
        );
        b.iter(|| {
            let pages: Vec<Page> = (0..4u8)
                .map(|l| session.read_page(l, &mut |_| {}).unwrap())
                .collect();
            black_box(Editor::new(&model, LayerSet::new(pages)).unwrap())
        });
    });
}

criterion_group!(benches, read_four_layers);
criterion_main!(benches);
