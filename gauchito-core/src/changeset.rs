//! Etherpad-style changesets for text editing.
//!
//! A [`ChangeSet`] describes how to transform a document of `in_len` chars into
//! one of `out_len` chars, using a sequence of [`Op`]s:
//!
//! - `Retain(n)` — keep `n` chars unchanged
//! - `Insert(s)` — insert string `s`
//! - `Delete(n)` — remove `n` chars

/// Cursor mapping bias when a position falls at an insert boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bias {
    /// Position stays before inserted text (remote edits).
    Before,
    /// Position lands after inserted text (local edits).
    After,
}

// ─────────────────────────────────────────────────────────────────────────────
// Op
// ─────────────────────────────────────────────────────────────────────────────

/// One step in a changeset. All lengths are in **chars** (not bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Keep `n` chars from the input.
    Retain(usize),
    /// Insert new text (does not consume input).
    Insert(String),
    /// Remove `n` chars from the input.
    Delete(usize),
}

// ─────────────────────────────────────────────────────────────────────────────
// ChangeSet
// ─────────────────────────────────────────────────────────────────────────────

/// Etherpad-style changeset: a sequence of retain/insert/delete ops that
/// transforms a document of `in_len` chars into one of `out_len` chars.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeSet {
    ops: Vec<Op>,
    in_len: usize,
    out_len: usize,
}

impl ChangeSet {
    /// Identity changeset — retains everything, changes nothing.
    pub fn identity(len: usize) -> Self {
        if len == 0 {
            return ChangeSet {
                ops: Vec::new(),
                in_len: 0,
                out_len: 0,
            };
        }

        ChangeSet {
            ops: vec![Op::Retain(len)],
            in_len: len,
            out_len: len,
        }
    }

    /// Apply this changeset to a rope.
    ///
    /// # Panics
    /// Panics if `self.in_len != doc.len_chars()`.
    pub fn apply(&self, doc: &mut ropey::Rope) {
        assert_eq!(
            self.in_len,
            doc.len_chars(),
            "ChangeSet in_len {} != doc len {}",
            self.in_len,
            doc.len_chars()
        );

        let mut pos = 0;

        for op in &self.ops {
            match op {
                Op::Retain(n) => pos += n,
                Op::Insert(s) => {
                    let len = s.chars().count();
                    doc.insert(pos, s);
                    pos += len;
                }
                Op::Delete(n) => {
                    doc.remove(pos..pos + n);
                }
            }
        }
    }

    /// Map a cursor position through this changeset.
    ///
    /// `Bias::Before` keeps the position before inserted text (remote edits).
    /// `Bias::After` moves it past the insert (authoring view).
    pub fn map_pos(&self, pos: usize, bias: Bias) -> usize {
        let mut in_pos = 0;
        let mut out_pos = 0;

        for op in &self.ops {
            match op {
                Op::Retain(n) => {
                    if pos < in_pos + n {
                        return out_pos + (pos - in_pos);
                    }
                    in_pos += n;
                    out_pos += n;
                }
                Op::Insert(s) => {
                    let len = s.chars().count();

                    if pos == in_pos && bias == Bias::Before {
                        return out_pos;
                    }

                    out_pos += len;
                }
                Op::Delete(n) => {
                    if pos < in_pos + n {
                        return out_pos;
                    }
                    in_pos += n;
                }
            }
        }

        out_pos + (pos - in_pos)
    }

    /// Compose two changesets: `a` (doc 0→1) then `b` (doc 1→2) into one (doc 0→2).
    ///
    /// # Panics
    /// Panics if `a.out_len != b.in_len`.
    pub fn compose(a: &ChangeSet, b: &ChangeSet) -> ChangeSet {
        assert_eq!(
            a.out_len, b.in_len,
            "compose: a.out_len {} != b.in_len {}",
            a.out_len, b.in_len
        );

        let mut result = CsBuilder::new();

        // Cursors into each op sequence, tracking partial consumption.
        let mut a_idx = 0;
        let mut a_off = 0; // chars already consumed from current a op
        let mut b_idx = 0;
        let mut b_off = 0;

        loop {
            let a_done = a_idx >= a.ops.len();
            let b_done = b_idx >= b.ops.len();
            if a_done && b_done {
                break;
            }

            // B inserts go directly to output — they don't consume A's output.
            if !b_done {
                if let Op::Insert(ref s) = b.ops[b_idx] {
                    let remaining = &s[s.char_indices().nth(b_off).map_or(s.len(), |(i, _)| i)..];
                    result.insert(remaining);
                    b_idx += 1;
                    b_off = 0;
                    continue;
                }
            }

            // A deletes go directly to input consumption — B never sees them.
            if !a_done {
                if let Op::Delete(n) = a.ops[a_idx] {
                    let remaining = n - a_off;
                    result.delete(remaining);
                    a_idx += 1;
                    a_off = 0;
                    continue;
                }
            }

            if a_done || b_done {
                break;
            }

            // Both ops consume from A's output / B's input. Take the minimum.
            match (&a.ops[a_idx], &b.ops[b_idx]) {
                // A retains, B retains — pass through.
                (Op::Retain(a_n), Op::Retain(b_n)) => {
                    let a_rem = a_n - a_off;
                    let b_rem = b_n - b_off;
                    let take = a_rem.min(b_rem);
                    result.retain(take);
                    a_off += take;
                    b_off += take;
                    if a_off == *a_n {
                        a_idx += 1;
                        a_off = 0;
                    }
                    if b_off == *b_n {
                        b_idx += 1;
                        b_off = 0;
                    }
                }

                // A inserts, B retains — A's insertion passes through.
                (Op::Insert(s), Op::Retain(b_n)) => {
                    let a_len = s.chars().count();
                    let a_rem = a_len - a_off;
                    let b_rem = b_n - b_off;
                    let take = a_rem.min(b_rem);

                    let start = s.char_indices().nth(a_off).map_or(s.len(), |(i, _)| i);
                    let end = s
                        .char_indices()
                        .nth(a_off + take)
                        .map_or(s.len(), |(i, _)| i);
                    result.insert(&s[start..end]);

                    a_off += take;
                    b_off += take;
                    if a_off == a_len {
                        a_idx += 1;
                        a_off = 0;
                    }
                    if b_off == *b_n {
                        b_idx += 1;
                        b_off = 0;
                    }
                }

                // A retains, B deletes — B deletes what A kept.
                (Op::Retain(a_n), Op::Delete(b_n)) => {
                    let a_rem = a_n - a_off;
                    let b_rem = b_n - b_off;
                    let take = a_rem.min(b_rem);
                    result.delete(take);
                    a_off += take;
                    b_off += take;
                    if a_off == *a_n {
                        a_idx += 1;
                        a_off = 0;
                    }
                    if b_off == *b_n {
                        b_idx += 1;
                        b_off = 0;
                    }
                }

                // A inserts, B deletes — cancel out.
                (Op::Insert(s), Op::Delete(b_n)) => {
                    let a_len = s.chars().count();
                    let a_rem = a_len - a_off;
                    let b_rem = b_n - b_off;
                    let take = a_rem.min(b_rem);
                    // Nothing emitted — inserted text is immediately deleted.
                    a_off += take;
                    b_off += take;
                    if a_off == a_len {
                        a_idx += 1;
                        a_off = 0;
                    }
                    if b_off == *b_n {
                        b_idx += 1;
                        b_off = 0;
                    }
                }

                // Delete+Retain and Delete+Delete can't happen here
                // because A deletes are handled above.
                _ => unreachable!(),
            }
        }

        result.build()
    }

    /// Compose a sequence of (forward, inverse) pairs into a single pair.
    /// Forward changesets compose left-to-right, inverses right-to-left.
    pub fn compose_vec(edits: &[(ChangeSet, ChangeSet)]) -> Option<(ChangeSet, ChangeSet)> {
        if edits.is_empty() {
            return None;
        }

        let mut forward = edits[0].0.clone();
        let mut inverse = edits[0].1.clone();

        for pair in &edits[1..] {
            forward = ChangeSet::compose(&forward, &pair.0);
            inverse = ChangeSet::compose(&pair.1, &inverse);
        }

        Some((forward, inverse))
    }

    /// Compute the inverse changeset.
    /// Applying `self.invert(original)` after `self` restores the document.
    pub fn invert(&self, original: &ropey::Rope) -> ChangeSet {
        let mut b = CsBuilder::new();
        let mut in_pos = 0;

        for op in &self.ops {
            match op {
                Op::Retain(n) => {
                    b.retain(*n);
                    in_pos += n;
                }
                Op::Insert(s) => {
                    b.delete(s.chars().count());
                }
                Op::Delete(n) => {
                    let text: String = original.slice(in_pos..in_pos + n).to_string();
                    b.insert(&text);
                    in_pos += n;
                }
            }
        }

        b.build()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CsBuilder — normalized ChangeSet construction (internal)
// ─────────────────────────────────────────────────────────────────────────────

/// Internal builder that merges adjacent same-type ops automatically.
struct CsBuilder {
    ops: Vec<Op>,
    in_len: usize,
    out_len: usize,
}

impl CsBuilder {
    fn new() -> Self {
        CsBuilder {
            ops: Vec::new(),
            in_len: 0,
            out_len: 0,
        }
    }

    fn retain(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        self.in_len += n;
        self.out_len += n;

        if let Some(Op::Retain(r)) = self.ops.last_mut() {
            *r += n;
        } else {
            self.ops.push(Op::Retain(n));
        }
    }

    fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let len = text.chars().count();
        self.out_len += len;

        if let Some(Op::Insert(s)) = self.ops.last_mut() {
            s.push_str(text);
        } else {
            self.ops.push(Op::Insert(text.to_string()));
        }
    }

    fn delete(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        self.in_len += n;

        if let Some(Op::Delete(d)) = self.ops.last_mut() {
            *d += n;
        } else {
            self.ops.push(Op::Delete(n));
        }
    }

    fn build(self) -> ChangeSet {
        ChangeSet {
            ops: self.ops,
            in_len: self.in_len,
            out_len: self.out_len,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ChangeBuilder — public cursor-based API
// ─────────────────────────────────────────────────────────────────────────────

/// Cursor-based builder for constructing a [`ChangeSet`] from a sequence of
/// operations at ascending positions in the original document.
///
/// ```text
/// let mut b = ChangeBuilder::new(doc_len);
/// for (from, to) in sorted_ranges {
///     b.advance_to(from);
///     b.delete(to - from);
///     b.insert("replacement");
/// }
/// let cs = b.finish();
/// ```
pub struct ChangeBuilder {
    b: CsBuilder,
    doc_len: usize,
    cursor: usize,
}

impl ChangeBuilder {
    pub fn new(doc_len: usize) -> Self {
        ChangeBuilder {
            b: CsBuilder::new(),
            doc_len,
            cursor: 0,
        }
    }

    /// Advance to `pos` in the input document, emitting a Retain for skipped chars.
    pub fn advance_to(&mut self, pos: usize) {
        assert!(
            pos >= self.cursor,
            "ChangeBuilder: advance_to({pos}) but cursor already at {}",
            self.cursor
        );
        assert!(
            pos <= self.doc_len,
            "ChangeBuilder: advance_to({pos}) > doc_len {}",
            self.doc_len
        );

        if pos > self.cursor {
            self.b.retain(pos - self.cursor);
            self.cursor = pos;
        }
    }

    /// Delete `n` chars from the input at the current position.
    pub fn delete(&mut self, n: usize) {
        assert!(
            self.cursor + n <= self.doc_len,
            "ChangeBuilder: delete({n}) at cursor {} exceeds doc_len {}",
            self.cursor,
            self.doc_len
        );

        if n > 0 {
            self.b.delete(n);
            self.cursor += n;
        }
    }

    /// Insert text at the current position (does not consume input).
    pub fn insert(&mut self, text: &str) {
        if !text.is_empty() {
            self.b.insert(text);
        }
    }

    /// Replace `n` input chars with `text`.
    pub fn replace(&mut self, n: usize, text: &str) {
        self.delete(n);
        self.insert(text);
    }

    /// Current output position — tracks where cursors land after applied ops.
    pub fn out_pos(&self) -> usize {
        self.b.out_len
    }

    /// Finish building, emitting a trailing Retain for any remaining input.
    pub fn finish(mut self) -> ChangeSet {
        if self.cursor < self.doc_len {
            self.b.retain(self.doc_len - self.cursor);
        }

        self.b.build()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    // Helper: apply a changeset to a string.
    fn apply_str(s: &str, cs: &ChangeSet) -> String {
        let mut rope = Rope::from_str(s);
        cs.apply(&mut rope);
        rope.to_string()
    }

    #[test]
    fn compose_two_inserts() {
        // "hello" → insert " world" → "hello world" → insert "!" → "hello world!"
        let mut b1 = ChangeBuilder::new(5);
        b1.advance_to(5);
        b1.insert(" world");
        let a = b1.finish(); // 5 → 11

        let mut b2 = ChangeBuilder::new(11);
        b2.advance_to(11);
        b2.insert("!");
        let b = b2.finish(); // 11 → 12

        let composed = ChangeSet::compose(&a, &b);
        assert_eq!(apply_str("hello", &composed), "hello world!");
    }

    #[test]
    fn compose_insert_then_delete() {
        // "abc" → insert "X" at 1 → "aXbc" → delete "X" at 1 → "abc"
        let mut b1 = ChangeBuilder::new(3);
        b1.advance_to(1);
        b1.insert("X");
        let a = b1.finish(); // 3 → 4

        let mut b2 = ChangeBuilder::new(4);
        b2.advance_to(1);
        b2.delete(1);
        let b = b2.finish(); // 4 → 3

        let composed = ChangeSet::compose(&a, &b);
        assert_eq!(apply_str("abc", &composed), "abc");
    }

    #[test]
    fn compose_delete_then_insert() {
        // "abcd" → delete "b" → "acd" → insert "X" at 1 → "aXcd"
        let mut b1 = ChangeBuilder::new(4);
        b1.advance_to(1);
        b1.delete(1);
        let a = b1.finish(); // 4 → 3

        let mut b2 = ChangeBuilder::new(3);
        b2.advance_to(1);
        b2.insert("X");
        let b = b2.finish(); // 3 → 4

        let composed = ChangeSet::compose(&a, &b);
        assert_eq!(apply_str("abcd", &composed), "aXcd");
    }

    #[test]
    fn compose_with_identity() {
        let mut b1 = ChangeBuilder::new(5);
        b1.advance_to(2);
        b1.insert("X");
        let a = b1.finish(); // 5 → 6

        let id = ChangeSet::identity(6);
        let composed = ChangeSet::compose(&a, &id);
        assert_eq!(apply_str("hello", &composed), apply_str("hello", &a));

        let id2 = ChangeSet::identity(5);
        let composed2 = ChangeSet::compose(&id2, &a);
        assert_eq!(apply_str("hello", &composed2), apply_str("hello", &a));
    }

    #[test]
    fn compose_round_trip_with_invert() {
        // Compose a changeset with its inverse → should be identity.
        let original = Rope::from_str("hello");

        let mut b = ChangeBuilder::new(5);
        b.advance_to(2);
        b.delete(1);
        b.insert("XY");
        let cs = b.finish(); // "hello" → "heXYlo"

        let inv = cs.invert(&original);
        let round_trip = ChangeSet::compose(&cs, &inv);
        assert_eq!(apply_str("hello", &round_trip), "hello");
    }

    #[test]
    fn compose_consecutive_char_inserts() {
        // Simulates typing "abc" one character at a time.
        // "" → "a" → "ab" → "abc"
        let mut b1 = ChangeBuilder::new(0);
        b1.insert("a");
        let cs1 = b1.finish(); // 0 → 1

        let mut b2 = ChangeBuilder::new(1);
        b2.advance_to(1);
        b2.insert("b");
        let cs2 = b2.finish(); // 1 → 2

        let mut b3 = ChangeBuilder::new(2);
        b3.advance_to(2);
        b3.insert("c");
        let cs3 = b3.finish(); // 2 → 3

        let ab = ChangeSet::compose(&cs1, &cs2);
        let abc = ChangeSet::compose(&ab, &cs3);
        assert_eq!(apply_str("", &abc), "abc");
    }

    #[test]
    fn compose_vec_groups_edits() {
        let original = Rope::from_str("hello");

        // Edit 1: insert "X" at 0 → "Xhello"
        let mut b1 = ChangeBuilder::new(5);
        b1.insert("X");
        let cs1 = b1.finish();
        let inv1 = cs1.invert(&original);

        // Edit 2: delete last char → "Xhell"
        let mut after1 = original.clone();
        cs1.apply(&mut after1);
        let mut b2 = ChangeBuilder::new(6);
        b2.advance_to(5);
        b2.delete(1);
        let cs2 = b2.finish();
        let inv2 = cs2.invert(&after1);

        let (forward, inverse) = ChangeSet::compose_vec(&[(cs1, inv1), (cs2, inv2)]).unwrap();

        // Forward: "hello" → "Xhell"
        assert_eq!(apply_str("hello", &forward), "Xhell");

        // Inverse: "Xhell" → "hello"
        assert_eq!(apply_str("Xhell", &inverse), "hello");
    }
}
