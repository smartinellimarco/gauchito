use super::*;
use crate::Edit;

fn apply(doc: &str, e: &Edit) -> String {
    let mut s = doc.to_string();
    let i = s
        .char_indices()
        .nth(e.start)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let j = s
        .char_indices()
        .nth(e.end)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    s.replace_range(i..j, &e.text);
    s
}

fn check_tp1(doc: &str, a: &Edit, b: &Edit) {
    let pa = apply(&apply(doc, a), &transform(b, a));
    let pb = apply(&apply(doc, b), &transform(a, b));
    assert_eq!(
        pa, pb,
        "\nTP1 violation on {:?}\n  a={:?}\n  b={:?}\n  path(a)={:?}\n  path(b)={:?}",
        doc, a, b, pa, pb
    );
}

// TP1 tests
#[test]
fn tp1_non_overlapping_inserts() {
    check_tp1(
        "hello world",
        &Edit::new(0, 0, "A".into()),
        &Edit::new(11, 11, "B".into()),
    );
}

#[test]
fn tp1_non_overlapping_deletes() {
    check_tp1(
        "hello world",
        &Edit::new(0, 2, "".into()),
        &Edit::new(6, 11, "".into()),
    );
}

#[test]
fn tp1_insert_before_delete() {
    check_tp1(
        "hello world",
        &Edit::new(0, 0, "XXX".into()),
        &Edit::new(6, 11, "".into()),
    );
}

#[test]
fn tp1_delete_before_insert() {
    check_tp1(
        "hello world",
        &Edit::new(0, 5, "".into()),
        &Edit::new(11, 11, "!!!".into()),
    );
}

#[test]
fn tp1_adjacent_inserts() {
    check_tp1(
        "hello",
        &Edit::new(2, 2, "A".into()),
        &Edit::new(2, 2, "B".into()),
    );
}

#[test]
fn tp1_overlapping_deletes() {
    check_tp1(
        "hello world",
        &Edit::new(3, 8, "".into()),
        &Edit::new(5, 10, "".into()),
    );
}

#[test]
fn tp1_one_inside_other() {
    check_tp1(
        "hello world",
        &Edit::new(2, 9, "".into()),
        &Edit::new(4, 6, "".into()),
    );
}

#[test]
fn tp1_identical_deletes() {
    check_tp1(
        "hello world",
        &Edit::new(5, 10, "".into()),
        &Edit::new(5, 10, "".into()),
    );
}

#[test]
fn tp1_replace_vs_insert() {
    check_tp1(
        "hello",
        &Edit::new(1, 4, "X".into()),
        &Edit::new(0, 0, "Y".into()),
    );
}

#[test]
fn tp1_overlapping_replaces() {
    check_tp1(
        "hello world",
        &Edit::new(2, 7, "AAA".into()),
        &Edit::new(4, 9, "BBB".into()),
    );
}

#[test]
fn tp1_insert_at_delete_boundary() {
    check_tp1(
        "hello world",
        &Edit::new(5, 5, "X".into()),
        &Edit::new(5, 8, "".into()),
    );
}

#[test]
fn tp1_delete_then_insert_same_spot() {
    check_tp1(
        "hello",
        &Edit::new(2, 4, "".into()),
        &Edit::new(2, 2, "X".into()),
    );
}

// transform unit tests

#[test]
fn xf_a_before_b() {
    let r = transform(&Edit::new(0, 2, "XX".into()), &Edit::new(5, 7, "YY".into()));
    assert_eq!((r.start, r.end), (0, 2));
}

#[test]
fn xf_a_after_insert() {
    let r = transform(
        &Edit::new(10, 12, "XX".into()),
        &Edit::new(5, 5, "YYY".into()),
    );
    assert_eq!((r.start, r.end), (13, 15));
}

#[test]
fn xf_a_after_delete() {
    let r = transform(&Edit::new(10, 12, "XX".into()), &Edit::new(5, 8, "".into()));
    assert_eq!((r.start, r.end), (7, 9));
}

#[test]
fn xf_a_inside_b_delete() {
    let r = transform(&Edit::new(6, 8, "XX".into()), &Edit::new(5, 10, "Y".into()));
    assert_eq!((r.start, r.end, r.text.as_str()), (6, 6, "XX"));
}

#[test]
fn xf_overlapping_deletes() {
    // a: [-----)    [3..8)
    // b:    [-----) [5..10)
    let r = transform(&Edit::new(3, 8, "".into()), &Edit::new(5, 10, "".into()));
    assert_eq!((r.start, r.end), (3, 5));
}

#[test]
fn xf_b_inside_a_delete() {
    let r = transform(&Edit::new(3, 10, "".into()), &Edit::new(5, 7, "".into()));
    assert_eq!((r.start, r.end), (3, 8));
}

#[test]
fn xf_identical_deletes() {
    let r = transform(&Edit::new(5, 10, "".into()), &Edit::new(5, 10, "".into()));
    assert_eq!((r.start, r.end), (5, 5));
}

#[test]
fn xf_a_extends_past_b() {
    // a:    [------) [7..15)
    // b: [-----)     [5..10) → "Y"
    let r = transform(
        &Edit::new(7, 15, "XX".into()),
        &Edit::new(5, 10, "Y".into()),
    );
    assert_eq!((r.start, r.end, r.text.as_str()), (6, 11, "XX"));
}

// transform_pos tests

#[test]
fn pos_before() {
    assert_eq!(transform_pos(3, &Edit::new(5, 8, "XX".into())), 3);
}

#[test]
fn pos_after_insert() {
    assert_eq!(transform_pos(10, &Edit::new(5, 5, "XXX".into())), 13);
}

#[test]
fn pos_after_delete() {
    assert_eq!(transform_pos(10, &Edit::new(5, 8, "".into())), 7);
}

#[test]
fn pos_inside_delete() {
    assert_eq!(transform_pos(7, &Edit::new(5, 10, "XX".into())), 7);
}

#[test]
fn pos_at_start() {
    assert_eq!(transform_pos(5, &Edit::new(5, 8, "XX".into())), 5);
}
