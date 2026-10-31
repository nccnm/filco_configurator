//! Random edit sessions: undoing every step gives back exactly the pages read from the keyboard, redoing every step
//! gives the edited pages, and the diff always lists exactly the slots that differ. Backups and export files of
//! random pages read back unchanged.

use filco_core::model::builtin;
use filco_core::{LayerSet, ModelDescriptor, Page, Slot, export};
use filco_service::discovery::InterfaceInfo;
use filco_service::editor::Editor;
use filco_service::store::{self, BackupStore};
use proptest::prelude::*;

fn model() -> ModelDescriptor {
    builtin::models().unwrap().remove(0)
}

fn pages() -> impl Strategy<Value = LayerSet> {
    proptest::collection::vec(proptest::collection::vec(any::<u8>(), 512), 4)
        .prop_map(|v| LayerSet::new(v.iter().map(|b| Page::from_bytes(b).unwrap()).collect()))
}

/// Slots the editor accepts: empty, keystrokes, clicks, consumer and system actions.
fn slot() -> impl Strategy<Value = Slot> {
    prop_oneof![
        Just(Slot([0; 4])),
        (any::<u8>(), 1u8..=0xff).prop_map(|(m, u)| Slot([2, m, u, 0])),
        (1u8..=0x1f).prop_map(|b| Slot([1, 1, b, 0])),
        (1u16..=0x3ff).prop_map(|u| {
            let [lo, hi] = u.to_le_bytes();
            Slot([3, lo, hi, 0])
        }),
        prop_oneof![Just(1u8), Just(2), Just(4)].prop_map(|u| Slot([4, u, 0, 0])),
    ]
}

#[derive(Debug, Clone)]
enum Op {
    Assign {
        layers: Vec<usize>,
        key: usize,
        slot: Slot,
    },
    Copy {
        from: usize,
        to: usize,
    },
    Undo,
    Redo,
}

fn op(keys: usize) -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (proptest::collection::vec(0usize..4, 1..4), 0..keys, slot())
            .prop_map(|(layers, key, slot)| Op::Assign { layers, key, slot }),
        1 => (0usize..4, 0usize..4).prop_map(|(from, to)| Op::Copy { from, to }),
        2 => Just(Op::Undo),
        1 => Just(Op::Redo),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn undo_and_redo_everything(base in pages(), ops in proptest::collection::vec(op(72), 1..40)) {
        let m = model();
        let mut ed = Editor::new(&m, base.clone()).unwrap();
        for o in &ops {
            match o {
                Op::Assign { layers, key, slot } => {
                    ed.assign_layers(&m, layers, m.layout.keys[*key].index, *slot).unwrap();
                }
                Op::Copy { from, to } => { ed.copy_layer(&m, *from, *to).unwrap(); }
                Op::Undo => { ed.undo(&m); }
                Op::Redo => { ed.redo(&m); }
            }
            // The diff is exactly the set of slots that differ, and only those.
            let changes = ed.changes(&m);
            for c in &changes {
                prop_assert_ne!(c.old, c.new);
                prop_assert_eq!(base.slot(m.addressing(), c.layer, c.index), Some(c.old));
                prop_assert_eq!(ed.working().slot(m.addressing(), c.layer, c.index), Some(c.new));
            }
            prop_assert_eq!(changes.is_empty(), ed.working() == &base);
        }
        // Steps undone at the end are still on the redo stack: the full history ends after all of them.
        while ed.redo(&m) {}
        let edited = ed.working().clone();
        while ed.undo(&m) {}
        prop_assert_eq!(ed.working(), &base);
        while ed.redo(&m) {}
        prop_assert_eq!(ed.working(), &edited);
    }

    #[test]
    fn backups_and_exports_round_trip(p in pages()) {
        let m = model();
        let tmp = tempfile::tempdir().unwrap();
        let info = InterfaceInfo {
            path: "sim".into(), vendor_id: 0x2efd, product_id: 0x1972, version: 0x72,
            usage_page: 1, input_len: 9, output_len: 2, feature_len: 65,
        };
        let dir = BackupStore::new(tmp.path()).save(&m, &info, &p, "test", "20261101-000000").unwrap();
        prop_assert_eq!(&store::load(&dir, &m).unwrap().1, &p);
        let text = export::to_json(&export::build(&m, &p, "0.1.0", "20261101-000000"));
        prop_assert_eq!(&export::parse(&text, &m).unwrap(), &p);
    }
}
