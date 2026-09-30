use gauchito_core::splice::{Gravity, Splice, phi};
use gauchito_core::{Buffer, BufferOptions};
use ropey::Rope;

const L: Gravity = Gravity::Left;
const R: Gravity = Gravity::Right;

fn buf(s: &str) -> Buffer {
    Buffer::new(Rope::from_str(s), None, BufferOptions::default())
}

#[test]
fn apply_pure_insert() {
    let mut b = buf("hello");
    let inv = b.apply(&Splice::new(2, 2, "XY".into()));
    assert_eq!(b.text.to_string(), "heXYllo");
    assert_eq!(inv, Splice::new(2, 4, "".into()));
}

#[test]
fn apply_pure_delete() {
    let mut b = buf("hello");
    let inv = b.apply(&Splice::new(1, 4, "".into()));
    assert_eq!(b.text.to_string(), "ho");
    assert_eq!(inv, Splice::new(1, 1, "ell".into()));
}

#[test]
fn apply_replace() {
    let mut b = buf("hello");
    let inv = b.apply(&Splice::new(1, 4, "XYZ".into()));
    assert_eq!(b.text.to_string(), "hXYZo");
    assert_eq!(inv, Splice::new(1, 4, "ell".into()));
}

#[test]
fn invert_roundtrips_insert() {
    let mut b = buf("hello");
    let inv = b.apply(&Splice::new(2, 2, "XY".into()));
    b.apply(&inv);
    assert_eq!(b.text.to_string(), "hello");
}

#[test]
fn invert_roundtrips_delete() {
    let mut b = buf("hello");
    let inv = b.apply(&Splice::new(1, 4, "".into()));
    b.apply(&inv);
    assert_eq!(b.text.to_string(), "hello");
}

#[test]
fn invert_roundtrips_replace() {
    let mut b = buf("hello");
    let inv = b.apply(&Splice::new(1, 4, "WORLD".into()));
    b.apply(&inv);
    assert_eq!(b.text.to_string(), "hello");
}

// Default-gravity phi tests use Right gravity — that's the historical
// behavior and the conservative default. See the `phi_left_gravity_*`
// tests below for the asymmetric cases.

#[test]
fn phi_pure_insert_right_gravity() {
    let m = Splice::new(5, 5, "XY".into());
    assert_eq!(phi(&m, 3, R), 3);
    assert_eq!(phi(&m, 5, R), 7);
    assert_eq!(phi(&m, 8, R), 10);
}

#[test]
fn phi_pure_delete_right_gravity() {
    let m = Splice::new(3, 7, "".into());
    assert_eq!(phi(&m, 2, R), 2);
    assert_eq!(phi(&m, 3, R), 3);
    assert_eq!(phi(&m, 5, R), 3);
    assert_eq!(phi(&m, 7, R), 3);
    assert_eq!(phi(&m, 9, R), 5);
}

#[test]
fn phi_replace_shrinks_right_gravity() {
    let m = Splice::new(3, 7, "AB".into());
    assert_eq!(phi(&m, 2, R), 2);
    assert_eq!(phi(&m, 3, R), 5);
    assert_eq!(phi(&m, 5, R), 5);
    assert_eq!(phi(&m, 7, R), 5);
    assert_eq!(phi(&m, 9, R), 7);
}

#[test]
fn phi_replace_grows_right_gravity() {
    let m = Splice::new(3, 5, "ABCD".into());
    assert_eq!(phi(&m, 2, R), 2);
    assert_eq!(phi(&m, 3, R), 7);
    assert_eq!(phi(&m, 4, R), 7);
    assert_eq!(phi(&m, 5, R), 7);
    assert_eq!(phi(&m, 8, R), 10);
}

// Left-gravity differs only inside `[s.p, s.q]`: it collapses to `s.p`
// rather than `s.p + n`. Outside that range, gravity is irrelevant.

#[test]
fn phi_pure_insert_left_gravity_stays() {
    let m = Splice::new(5, 5, "XY".into());
    assert_eq!(phi(&m, 5, L), 5, "insert at pin → left gravity keeps pin in place");
    assert_eq!(phi(&m, 8, L), 10, "outside range: gravity-independent");
}

#[test]
fn phi_replace_left_gravity_collapses_left() {
    let m = Splice::new(3, 7, "AB".into());
    assert_eq!(phi(&m, 3, L), 3);
    assert_eq!(phi(&m, 5, L), 3);
    assert_eq!(phi(&m, 7, L), 3);
}

#[test]
#[should_panic]
fn new_rejects_q_less_than_p() {
    let _ = Splice::new(5, 3, "".into());
}

#[test]
fn phi_author_gravity_follows_local_insert() {
    let m = Splice::new(5, 5, "XY".into());
    assert_eq!(phi(&m, 5, Gravity::Author), 7);
}

#[test]
fn phi_author_gravity_stays_on_remote_insert() {
    let m = Splice::remote(5, 5, "XY".into());
    assert_eq!(phi(&m, 5, Gravity::Author), 5);
    assert_eq!(phi(&m, 8, Gravity::Author), 10);
}

#[test]
fn apply_counts_bytes_and_inverse_restores_multibyte_text() {
    let mut b = buf("añb");
    let inverse = b.apply(&Splice::new(1, 3, "üé".into()));
    assert_eq!(b.text.to_string(), "aüéb");
    assert_eq!(inverse, Splice::new(1, 5, "ñ".into()));
    b.apply(&inverse);
    assert_eq!(b.text.to_string(), "añb");
}

#[test]
fn phi_shifts_by_byte_length() {
    let m = Splice::new(0, 0, "ñ".into());
    assert_eq!(phi(&m, 3, R), 5);
}
