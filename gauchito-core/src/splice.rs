//! Splice — the edit primitive — and ϕ, its position projection.
//!
//! A `Splice(p, q, text)` says "replace `[p, q)` with `text`". It's
//! the unit of edit and the unit of history. [`Splice::apply`] is
//! `pub(crate)`: [`crate::Buffer::apply`] is the only path that
//! touches the rope, and the "apply ⇒ pins project" invariant lives
//! or dies on that.
//!
//! [`phi`] is the projection: where does offset `r` land after splice
//! `s`? Outside the touched range, shift by `text.len - (q-p)`.
//! Inside the deleted region, `r` collapses to `s.p` (Left gravity)
//! or `s.p + n` (Right gravity).
//!
//! Per-pin gravity is non-negotiable: without it, decorations can't
//! distinguish "swallow inserts at my edge" from "don't"; virtual
//! text has no position semantics; Helix-style extending selections
//! are inexpressible. One bit per pin, one branch in ϕ — trivial
//! against the breaking shape change later.

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
        Self { p, q, text, origin: Origin::Local }
    }

    pub fn remote(p: usize, q: usize, text: String) -> Self {
        Self { origin: Origin::Remote, ..Self::new(p, q, text) }
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
