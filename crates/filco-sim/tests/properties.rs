//! Any page survives the wire: written through the protocol to the simulated keyboard, stored, and read back, it
//! comes out byte for byte, and the checksum the keyboard computes always matches the host's.

use filco_core::Page;
use filco_protocol::{Delays, Session, VirtualClock};
use filco_sim::SimKeyboard;
use proptest::prelude::*;

fn page() -> impl Strategy<Value = Page> {
    proptest::collection::vec(any::<u8>(), 512).prop_map(|b| Page::from_bytes(&b).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn write_then_read_round_trips(pages in proptest::collection::vec(page(), 4), layer in 0u8..4) {
        let mut s = Session::new(SimKeyboard::blank(4), VirtualClock::default(), Delays::FILCO_ASSIST);
        let target = &pages[usize::from(layer)];
        s.write_page(layer, target, &mut |_| {}).unwrap();
        prop_assert_eq!(&s.read_page(layer, &mut |_| {}).unwrap(), target);
        // Only that layer changed in flash.
        let (sim, _) = s.into_parts();
        for (l, p) in sim.flash().iter().enumerate() {
            if l == usize::from(layer) {
                prop_assert_eq!(p, target);
            } else {
                prop_assert_eq!(p, &Page::empty());
            }
        }
    }
}
