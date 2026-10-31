//! Listing HID interfaces on this machine (SetupAPI, open with zero access, attributes and caps). No report is sent
//! to any device. Budget (PLAN.md section 8): at most 50 ms.
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

fn enumerate(c: &mut Criterion) {
    c.bench_function("enumerate HID interfaces", |b| {
        b.iter(|| black_box(filco_hid_windows::enumerate().unwrap()));
    });
}

criterion_group!(benches, enumerate);
criterion_main!(benches);
