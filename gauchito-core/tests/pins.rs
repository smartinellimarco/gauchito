use gauchito_core::pins::PinTable;
use gauchito_core::splice::{Gravity, Splice};

#[test]
fn add_and_resolve() {
    let mut t = PinTable::new();
    let a = t.add(5, Gravity::Right);
    let b = t.add(10, Gravity::Right);

    assert_eq!(t.offset(a), 5);
    assert_eq!(t.offset(b), 10);
    assert_eq!(t.len(), 2);
}

#[test]
fn set_offset_mutates_in_place() {
    let mut t = PinTable::new();
    let a = t.add(5, Gravity::Right);

    t.set_offset(a, 42);

    assert_eq!(t.offset(a), 42);
}

#[test]
fn apply_splice_insert_shifts_after_point() {
    let mut t = PinTable::new();
    let before = t.add(2, Gravity::Right);
    let after = t.add(8, Gravity::Right);

    t.apply(&Splice::new(5, 5, "XX".into()));

    assert_eq!(t.offset(before), 2);
    assert_eq!(t.offset(after), 10);
}

#[test]
fn apply_splice_delete_clamps_and_shifts() {
    let mut t = PinTable::new();
    let outside = t.add(2, Gravity::Right);
    let inside = t.add(7, Gravity::Right);
    let after = t.add(10, Gravity::Right);

    t.apply(&Splice::new(5, 8, String::new()));

    assert_eq!(t.offset(outside), 2);
    assert_eq!(t.offset(inside), 5);
    assert_eq!(t.offset(after), 7);
}

#[test]
fn inverse_splice_undoes_anchor_movement() {
    use gauchito_core::{Buffer, BufferOptions};
    use ropey::Rope;

    // Anchors live inside a Buffer's pin table in practice; here we
    // exercise an external PinTable against splices applied to a
    // Buffer (since Buffer::apply is the only public path that runs
    // a splice on text).
    let mut buf = Buffer::new(Rope::from_str("abcdef"), None, BufferOptions::default());

    let mut t = PinTable::new();
    let id = t.add(3, Gravity::Right);

    let splice = Splice::new(1, 1, "ZZ".into());
    let inverse = buf.apply(&splice);

    t.apply(&splice);
    assert_eq!(t.offset(id), 5);

    t.apply(&inverse);
    assert_eq!(t.offset(id), 3);
}

#[test]
fn apply_leaves_pins_before_splice_untouched() {
    let mut t = PinTable::new();
    let p0 = t.add(0, Gravity::Right);
    let p3 = t.add(3, Gravity::Right);
    let p5 = t.add(5, Gravity::Right);

    t.apply(&Splice::new(10, 10, "ABCDE".into()));

    assert_eq!(t.offset(p0), 0);
    assert_eq!(t.offset(p3), 3);
    assert_eq!(t.offset(p5), 5);
}

#[test]
fn pins_in_returns_offsets_in_range() {
    let mut t = PinTable::new();
    let a = t.add(0, Gravity::Right);
    let b = t.add(5, Gravity::Right);
    let c = t.add(10, Gravity::Right);
    let d = t.add(20, Gravity::Right);

    let got: Vec<_> = t.pins_in(5, 15).collect();
    assert_eq!(got, vec![b, c]);
    assert!(!got.contains(&a));
    assert!(!got.contains(&d));
}

#[test]
fn pins_in_is_half_open() {
    let mut t = PinTable::new();
    let a = t.add(5, Gravity::Right);
    let _b = t.add(10, Gravity::Right);

    // [5, 10) — includes 5, excludes 10.
    let got: Vec<_> = t.pins_in(5, 10).collect();
    assert_eq!(got, vec![a]);
}

#[test]
fn mixed_gravity_inside_splice_rebuckets() {
    // Pins with offsets in [p,q] but opposite gravity cross each other
    // under φ: Left → s.p, Right → s.p+n. The order index has to
    // reflect the new offsets, not the originals.
    let mut t = PinTable::new();
    let right_at_3 = t.add(3, Gravity::Right);
    let left_at_5 = t.add(5, Gravity::Left);
    let right_at_7 = t.add(7, Gravity::Right);
    let left_at_9 = t.add(9, Gravity::Left);

    // Replace [2, 10) with "X" — n=1, o=8.
    t.apply(&Splice::new(2, 10, "X".into()));

    assert_eq!(t.offset(left_at_5), 2);
    assert_eq!(t.offset(left_at_9), 2);
    assert_eq!(t.offset(right_at_3), 3);
    assert_eq!(t.offset(right_at_7), 3);

    use std::collections::HashSet;
    let lefts: HashSet<_> = t.pins_in(2, 3).collect();
    assert_eq!(lefts, HashSet::from([left_at_5, left_at_9]));

    let rights: HashSet<_> = t.pins_in(3, 4).collect();
    assert_eq!(rights, HashSet::from([right_at_3, right_at_7]));
}

#[test]
fn remove_keeps_index_consistent() {
    let mut t = PinTable::new();
    let a = t.add(5, Gravity::Right);
    let b = t.add(10, Gravity::Right);
    let c = t.add(15, Gravity::Right);

    t.remove(b);

    let got: Vec<_> = t.pins_in(0, 100).collect();
    assert_eq!(got, vec![a, c]);
    assert_eq!(t.len(), 2);
}

#[test]
fn set_offset_repositions_in_index() {
    let mut t = PinTable::new();
    let a = t.add(5, Gravity::Right);
    let b = t.add(10, Gravity::Right);
    let c = t.add(15, Gravity::Right);

    // Move b past c.
    t.set_offset(b, 20);

    let got: Vec<_> = t.pins_in(0, 100).collect();
    assert_eq!(got, vec![a, c, b]);
}

#[test]
fn apply_preserves_tail_order() {
    // Pins past s.q shift by a uniform delta; their relative order
    // is invariant under φ.
    let mut t = PinTable::new();
    let p15 = t.add(15, Gravity::Right);
    let p20 = t.add(20, Gravity::Right);
    let p25 = t.add(25, Gravity::Right);

    t.apply(&Splice::new(0, 10, "AB".into()));

    assert_eq!(t.offset(p15), 7);
    assert_eq!(t.offset(p20), 12);
    assert_eq!(t.offset(p25), 17);

    let got: Vec<_> = t.pins_in(0, 100).collect();
    assert_eq!(got, vec![p15, p20, p25]);
}

#[test]
fn author_pins_rebucket_by_origin() {
    let mut t = PinTable::new();
    let cursor = t.add(5, Gravity::Author);
    let left = t.add(5, Gravity::Left);
    let right = t.add(5, Gravity::Right);

    t.apply(&Splice::remote(5, 5, "XY".into()));
    assert_eq!((t.offset(cursor), t.offset(left), t.offset(right)), (5, 5, 7));

    t.apply(&Splice::new(5, 5, "Z".into()));
    assert_eq!((t.offset(cursor), t.offset(left), t.offset(right)), (6, 5, 8));

    t.remove(cursor);
    t.remove(left);
    t.remove(right);
    assert!(t.is_empty());
    assert_eq!(t.pins_in(0, 100).count(), 0);
}
