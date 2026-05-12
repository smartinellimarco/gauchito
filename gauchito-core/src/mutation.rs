//! Text mutation primitive: splice `[p, q) → t` on a rope.
//!
//! A `Mutation` is one splice. Pure-insert is `q == p`; pure-delete is
//! `t == ""`; a true replace is both. `apply` mutates the rope and returns
//! the inverse 'Mutation'.
//!
//! `phi(m, r)` is the position-adjustment function ϕ — pure arithmetic on
//! the mutation's `(p, q, |t|)`. It tells anchor walkers and fold-style
//! threading where a pre-edit position lands post-edit.

use ropey::Rope;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutation {
    p: usize,
    q: usize,
    t: String,
}

impl Mutation {
    pub fn new(p: usize, q: usize, t: String) -> Self {
        assert!(p <= q, "Mutation::new: p ({p}) must be <= q ({q})");

        Self { p, q, t }
    }

    /// Splice: remove `[p, q)` and insert `t` at `p`. Returns the removed
    /// text `d` so the caller can build the inverse with `invert(d)`.
    pub fn apply(&self, rope: &mut Rope) -> Mutation {
        let d = if self.q > self.p {
            let s = rope.slice(self.p..self.q).to_string();

            rope.remove(self.p..self.q);

            s
        } else {
            String::new()
        };

        if !self.t.is_empty() {
            rope.insert(self.p, &self.t);
        }

        Mutation {
            p: self.p,
            q: self.p + self.t.chars().count(),
            t: d,
        }
    }
}

/// Position-adjustment function ϕ for the splice `(p, q, t)`.
///
/// - `r < p`         → `r`              (untouched)
/// - `p ≤ r ≤ q`     → `p + n`          (clamps to end of replacement)
/// - `r > q`         → `r + n − o`      (shift by net length change)
pub fn phi(m: &Mutation, r: usize) -> usize {
    let n = m.t.chars().count(); // |t|
    let o = m.q - m.p; // |[p, q)|

    if r < m.p {
        r
    } else if r <= m.q {
        m.p + n
    } else if n >= o {
        r + (n - o)
    } else {
        r - (o - n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_pure_insert() {
        let mut rope = Rope::from_str("hello");
        let m = Mutation::new(2, 2, "XY".into());
        let d = m.apply(&mut rope);
        assert_eq!(rope.to_string(), "heXYllo");
        assert_eq!(d, "");
    }

    #[test]
    fn apply_pure_delete() {
        let mut rope = Rope::from_str("hello");
        let m = Mutation::new(1, 4, "".into());
        let d = m.apply(&mut rope);
        assert_eq!(rope.to_string(), "ho");
        assert_eq!(d, "ell");
    }

    #[test]
    fn apply_replace() {
        let mut rope = Rope::from_str("hello");
        let m = Mutation::new(1, 4, "XYZ".into());
        let d = m.apply(&mut rope);
        assert_eq!(rope.to_string(), "hXYZo");
        assert_eq!(d, "ell");
    }

    #[test]
    fn invert_roundtrips_insert() {
        let mut rope = Rope::from_str("hello");
        let m = Mutation::new(2, 2, "XY".into());
        let d = m.apply(&mut rope);
        m.invert(d).apply(&mut rope);
        assert_eq!(rope.to_string(), "hello");
    }

    #[test]
    fn invert_roundtrips_delete() {
        let mut rope = Rope::from_str("hello");
        let m = Mutation::new(1, 4, "".into());
        let d = m.apply(&mut rope);
        m.invert(d).apply(&mut rope);
        assert_eq!(rope.to_string(), "hello");
    }

    #[test]
    fn invert_roundtrips_replace() {
        let mut rope = Rope::from_str("hello");
        let m = Mutation::new(1, 4, "WORLD".into());
        let d = m.apply(&mut rope);
        m.invert(d).apply(&mut rope);
        assert_eq!(rope.to_string(), "hello");
    }

    #[test]
    fn phi_pure_insert() {
        // Insert "XY" at 5 (q == p == 5, n=2, o=0).
        let m = Mutation::new(5, 5, "XY".into());
        assert_eq!(phi(&m, 3), 3); // r < p: unchanged
        assert_eq!(phi(&m, 5), 7); // r = p = q: → p + n
        assert_eq!(phi(&m, 8), 10); // r > q: → r + n - o = 8 + 2
    }

    #[test]
    fn phi_pure_delete() {
        // Delete [3, 7) (n=0, o=4).
        let m = Mutation::new(3, 7, "".into());
        assert_eq!(phi(&m, 2), 2); // before
        assert_eq!(phi(&m, 3), 3); // r = p: → p + n = 3
        assert_eq!(phi(&m, 5), 3); // inside: → p + n = 3
        assert_eq!(phi(&m, 7), 3); // r = q: → p + n = 3
        assert_eq!(phi(&m, 9), 5); // after: → r - (o - n) = 9 - 4
    }

    #[test]
    fn phi_replace_shrinks() {
        // Replace [3, 7) with "AB" (n=2, o=4).
        let m = Mutation::new(3, 7, "AB".into());
        assert_eq!(phi(&m, 2), 2); // before
        assert_eq!(phi(&m, 3), 5); // r = p: after-bias → p + n
        assert_eq!(phi(&m, 5), 5); // inside: clamps to p + n
        assert_eq!(phi(&m, 7), 5); // r = q: → p + n
        assert_eq!(phi(&m, 9), 7); // after: r - (o - n) = 9 - 2
    }

    #[test]
    fn phi_replace_grows() {
        // Replace [3, 5) with "ABCD" (n=4, o=2).
        let m = Mutation::new(3, 5, "ABCD".into());
        assert_eq!(phi(&m, 2), 2);
        assert_eq!(phi(&m, 3), 7); // p + n = 3 + 4
        assert_eq!(phi(&m, 4), 7); // inside
        assert_eq!(phi(&m, 5), 7); // r = q
        assert_eq!(phi(&m, 8), 10); // after: r + (n - o) = 8 + 2
    }

    #[test]
    #[should_panic]
    fn new_rejects_q_less_than_p() {
        let _ = Mutation::new(5, 3, "".into());
    }
}
