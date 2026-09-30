use ropey::Rope;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gravity {
    Left,
    Right,
    // Right for our own edits, Left for a peer's: typing moves the cursor, a peer typing at it doesn't.
    Author,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Local,
    Remote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    p: usize,
    q: usize,
    text: String,
    origin: Origin,
}

impl Splice {
    pub fn new(p: usize, q: usize, text: String) -> Self {
        assert!(p <= q, "Splice::new: p ({p}) must be <= q ({q})");
        Self {
            p,
            q,
            text,
            origin: Origin::Local,
        }
    }

    pub fn remote(p: usize, q: usize, text: String) -> Self {
        Self {
            origin: Origin::Remote,
            ..Self::new(p, q, text)
        }
    }

    pub fn origin(&self) -> Origin {
        self.origin
    }

    pub fn p(&self) -> usize {
        self.p
    }

    pub fn q(&self) -> usize {
        self.q
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_identity(&self) -> bool {
        self.p == self.q && self.text.is_empty()
    }

    pub(crate) fn apply(&self, rope: &mut Rope) -> Splice {
        let mut removed = String::new();

        if self.q > self.p {
            removed = rope.slice(self.p..self.q).to_string();

            rope.remove(self.p..self.q);
        }

        if !self.text.is_empty() {
            rope.insert(self.p, &self.text);
        }

        Splice::new(self.p, self.p + self.text.len(), removed)
    }
}

pub fn phi(s: &Splice, r: usize, gravity: Gravity) -> usize {
    let n = s.text.len();
    let o = s.q - s.p;

    if r < s.p {
        r
    } else if r <= s.q {
        match (gravity, s.origin) {
            (Gravity::Left, _) | (Gravity::Author, Origin::Remote) => s.p,
            (Gravity::Right, _) | (Gravity::Author, Origin::Local) => s.p + n,
        }
    } else if n >= o {
        r + (n - o)
    } else {
        r - (o - n)
    }
}

#[cfg(test)]
mod tests {
    use crate::Buffer;
    use crate::splice::{Gravity, Splice, phi};
    use ropey::Rope;

    const L: Gravity = Gravity::Left;
    const R: Gravity = Gravity::Right;

    fn buf(s: &str) -> Buffer {
        let mut b = Buffer::new();
        b.text = Rope::from_str(s);
        b
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

    #[test]
    fn phi_pure_insert_left_gravity_stays() {
        let m = Splice::new(5, 5, "XY".into());
        assert_eq!(
            phi(&m, 5, L),
            5,
            "insert at pin → left gravity keeps pin in place"
        );
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
}
