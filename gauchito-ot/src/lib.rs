//! Operational Transformation (OT) for concurrent text edits.
//!
//! Implements the two-way synchronisation algorithm from the Jupiter
//! collaboration system, using **Etherpad-style changesets** (retain/insert/delete).
//!
//! # Model
//!
//! A [`ChangeSet`] describes how to transform a document of `in_len` chars into
//! one of `out_len` chars, using a sequence of [`Op`]s:
//!
//! - `Retain(n)` — keep `n` chars unchanged
//! - `Insert(s)` — insert string `s`
//! - `Delete(n)` — remove `n` chars
//!
//! [`cs_xform`] transforms a pair of *concurrent* changesets `(a, b)` — both
//! generated against the **same** document state — into `(a′, b′)` such that
//!
//! ```text
//! apply(apply(doc, a), b′)  ==  apply(apply(doc, b), a′)
//! ```
//!
//! Tie-breaking (both inserting at the same position) follows Jupiter:
//! **server (b) text is placed first**. TODO: add more tests for this.
//! sources:
//! - https://dl.acm.org/doi/10.1145/215585.215706
//! - https://docs.etherpad.org

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ─────────────────────────────────────────────────────────────────────────────
// Op
// ─────────────────────────────────────────────────────────────────────────────

/// One step in a changeset.  All lengths are in **chars** (not bytes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    /// Keep `n` chars from the input.
    Retain(usize),
    /// Insert new text (not consuming input).
    Insert(String),
    /// Remove `n` chars from the input.
    Delete(usize),
}

// ─────────────────────────────────────────────────────────────────────────────
// ChangeSet
// ─────────────────────────────────────────────────────────────────────────────

/// Etherpad-style changeset: a sequence of retain/insert/delete ops that
/// transforms a document of `in_len` chars into one of `out_len` chars.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub ops: Vec<Op>,
    pub in_len: usize,
    pub out_len: usize,
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

    /// Map a cursor position through this changeset ("before" bias — positions
    /// at an insert boundary stay before the inserted text).
    pub fn map_pos(&self, pos: usize) -> usize {
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
                    if pos == in_pos {
                        return out_pos; // "before" bias
                    }
                    out_pos += len;
                }
                Op::Delete(n) => {
                    if pos < in_pos + n {
                        return out_pos; // inside deleted region → collapse
                    }
                    in_pos += n;
                }
            }
        }
        out_pos + (pos - in_pos)
    }

    /// Compute the inverse: applying `self.invert(original)` after `self`
    /// restores the document.  `original` is the rope **before** this changeset.
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
// CsBuilder — normalized ChangeSet construction
// ─────────────────────────────────────────────────────────────────────────────

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
// ChangeBuilder — public cursor-based API for building changesets
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
///     // b.out_pos() is the cursor position in the output
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

    /// Advance to position `pos` in the input document, emitting a Retain.
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

    /// Current output position — use this to track where cursors land.
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
// cs_xform — the core OT function (Etherpad dual-cursor algorithm)
// ─────────────────────────────────────────────────────────────────────────────

/// Transform two concurrent changesets into a convergent pair.
///
/// `a` is the **client** (local) changeset; `b` is the **server** (remote).
/// Both were generated against the same document state (`a.in_len == b.in_len`).
///
/// Returns `(a′, b′)` such that:
///
/// ```text
/// apply(apply(doc, a), b′)  ==  apply(apply(doc, b), a′)
/// ```
///
/// ## Tie-breaking
/// When both changesets insert at the same input position, server (b) text is
/// placed **first** (Jupiter §7).
/// Transform two concurrent changesets into a convergent pair.
///
/// When `a_wins_ties` is true, `a`'s inserts are placed **before** `b`'s at the
/// same input position.  When false, `b`'s inserts go first.
///
/// In Jupiter: the **server** (higher-priority site) should always win ties.
/// - Server-side session (processing client msg): `a_wins_ties = true`
///   (saved = server-local, incoming = client-remote → local/a wins)
/// - Client-side session (processing server echo): `a_wins_ties = false`
///   (saved = client-local, incoming = server-remote → remote/b wins)
pub fn cs_xform(a: &ChangeSet, b: &ChangeSet, a_wins_ties: bool) -> (ChangeSet, ChangeSet) {
    assert_eq!(
        a.in_len, b.in_len,
        "cs_xform: in_len mismatch ({} vs {})",
        a.in_len, b.in_len
    );

    let mut a_prime = CsBuilder::new();
    let mut b_prime = CsBuilder::new();

    let mut ai = 0usize;
    let mut bi = 0usize;
    let mut a_off = 0usize;
    let mut b_off = 0usize;

    loop {
        let a_done = ai >= a.ops.len();
        let b_done = bi >= b.ops.len();
        if a_done && b_done {
            break;
        }

        // Handle inserts.  The winner goes first when both are Insert.
        let a_is_insert = !a_done && matches!(a.ops[ai], Op::Insert(_));
        let b_is_insert = !b_done && matches!(b.ops[bi], Op::Insert(_));

        // Decide which insert to process first.
        // TODO: document when/why this is needed (real time / overlapping inserts conflict) and compare with Edit {start, end, text}
        let do_a_insert = a_is_insert && (!b_is_insert || a_wins_ties);
        let do_b_insert = b_is_insert && !do_a_insert;

        if do_b_insert && let Op::Insert(ref s) = b.ops[bi] {
            let len = s.chars().count();
            b_prime.insert(s);
            a_prime.retain(len);
            bi += 1;
            b_off = 0;
            continue;
        }

        if (do_a_insert || a_is_insert)
            && let Op::Insert(ref s) = a.ops[ai]
        {
            let len = s.chars().count();
            a_prime.insert(s);
            b_prime.retain(len);
            ai += 1;
            a_off = 0;
            continue;
        }

        // Both sides exhausted their inserts at this point.
        if a_done || b_done {
            // Shouldn't happen if in_len matches, but just in case
            break;
        }

        // Both are consuming input (Retain or Delete).
        let a_rem = op_input_len(&a.ops[ai]) - a_off;
        let b_rem = op_input_len(&b.ops[bi]) - b_off;
        let n = a_rem.min(b_rem);

        match (&a.ops[ai], &b.ops[bi]) {
            (Op::Retain(_), Op::Retain(_)) => {
                a_prime.retain(n);
                b_prime.retain(n);
            }
            (Op::Delete(_), Op::Retain(_)) => {
                // a deletes chars that b retained → a′ still deletes.
                // b′: these chars won't exist in a's output, so nothing.
                a_prime.delete(n);
            }
            (Op::Retain(_), Op::Delete(_)) => {
                // b deletes chars that a retained → b′ still deletes.
                // a′: these chars won't exist in b's output, so nothing.
                b_prime.delete(n);
            }
            (Op::Delete(_), Op::Delete(_)) => {
                // Both delete the same chars — no-op in both primes.
            }
            _ => unreachable!("inserts handled above"),
        }

        a_off += n;
        b_off += n;
        if a_off >= op_input_len(&a.ops[ai]) {
            ai += 1;
            a_off = 0;
        }
        if b_off >= op_input_len(&b.ops[bi]) {
            bi += 1;
            b_off = 0;
        }
    }

    (a_prime.build(), b_prime.build())
}

/// How many input chars an op consumes (0 for Insert).
fn op_input_len(op: &Op) -> usize {
    match op {
        Op::Retain(n) | Op::Delete(n) => *n,
        Op::Insert(_) => 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Jupiter session state
// ─────────────────────────────────────────────────────────────────────────────

/// Keeps track of the link state between two sites (Client ↔ Server).
pub struct JupiterSession {
    /// Count of messages generated locally and sent.
    pub k: u64,
    /// Count of messages received and processed from the other side.
    pub y: u64,
    /// Queue of locally-generated changesets not yet acknowledged.
    pub outgoing: VecDeque<SentOp>,
    /// If true, this site's local ops win insert ties (server-side sessions).
    /// If false, the remote side's ops win (client-side sessions).
    local_wins_ties: bool,
}

pub struct SentOp {
    pub cs: ChangeSet,
    pub k: u64,
}

impl Default for JupiterSession {
    fn default() -> Self {
        Self::new()
    }
}

impl JupiterSession {
    /// Create a **client-side** session (server wins ties).
    pub fn new() -> Self {
        Self {
            k: 0,
            y: 0,
            outgoing: VecDeque::new(),
            local_wins_ties: false,
        }
    }

    /// Create a **server-side** session (this site wins ties).
    pub fn new_server() -> Self {
        Self {
            k: 0,
            y: 0,
            outgoing: VecDeque::new(),
            local_wins_ties: true,
        }
    }

    /// Enqueue a locally-generated changeset.
    /// Returns `(changeset, k, y)` — the tags for the wire message.
    pub fn push_local(&mut self, cs: ChangeSet) -> (ChangeSet, u64, u64) {
        let tag = (self.k, self.y);
        self.outgoing.push_back(SentOp {
            cs: cs.clone(),
            k: self.k,
        });
        self.k += 1;
        (cs, tag.0, tag.1)
    }

    /// Process one incoming changeset from the other side.
    /// Returns the transformed changeset to apply to the local document.
    ///
    /// Jupiter Fig. 6:
    ///   1. Discard outgoing entries already acknowledged (k < remote_y).
    ///   2. Transform incoming against each remaining outgoing entry.
    ///   3. Increment y.
    pub fn push_remote(&mut self, incoming: ChangeSet, remote_k: u64, remote_y: u64) -> ChangeSet {
        debug_assert_eq!(
            remote_k, self.y,
            "out-of-order (remote_k={remote_k} != self.y={})",
            self.y
        );

        // 1. Prune ACK'd entries.
        while self.outgoing.front().is_some_and(|f| f.k < remote_y) {
            self.outgoing.pop_front();
        }

        // 2. Transform against each remaining outgoing op.
        let mut inc = incoming;
        for saved in &mut self.outgoing {
            let (saved_prime, inc_prime) = cs_xform(&saved.cs, &inc, self.local_wins_ties);
            saved.cs = saved_prime;
            inc = inc_prime;
        }

        self.y += 1;
        inc
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    // ── Helper: build a ChangeSet from (start, end, text) ────────────────────

    fn cs(doc_len: usize, start: usize, end: usize, text: &str) -> ChangeSet {
        let mut b = ChangeBuilder::new(doc_len);
        b.advance_to(start);
        b.replace(end - start, text);
        b.finish()
    }

    fn cs_ins(doc_len: usize, pos: usize, text: &str) -> ChangeSet {
        cs(doc_len, pos, pos, text)
    }

    fn cs_del(doc_len: usize, start: usize, end: usize) -> ChangeSet {
        cs(doc_len, start, end, "")
    }

    // ── Convergence helper ───────────────────────────────────────────────────

    /// Both OT paths must produce the same document.
    fn converges(doc_str: &str, c: ChangeSet, s: ChangeSet) -> String {
        // Server (s) wins ties → a_wins_ties=false (s is b).
        let (c_prime, s_prime) = cs_xform(&c, &s, false);

        let mut doc1 = ropey::Rope::from_str(doc_str);
        let mut doc2 = ropey::Rope::from_str(doc_str);

        c.apply(&mut doc1);
        s_prime.apply(&mut doc1);

        s.apply(&mut doc2);
        c_prime.apply(&mut doc2);

        assert_eq!(doc1, doc2, "convergence failed for c={:?} s={:?}", c, s);
        doc1.to_string()
    }

    // ── Jupiter 2-client test ────────────────────────────────────────────────

    #[test]
    fn test_jupiter_convergence() {
        let mut server_doc = ropey::Rope::from_str("ABCDE");
        let mut client_a_doc = ropey::Rope::from_str("ABCDE");
        let mut client_b_doc = ropey::Rope::from_str("ABCDE");

        let mut server_session_a = JupiterSession::new_server();
        let mut server_session_b = JupiterSession::new_server();
        let mut client_a_session = JupiterSession::new();
        let mut client_b_session = JupiterSession::new();

        // 1. Client A inserts 'X' at 2
        let cs_a = cs_ins(5, 2, "X");
        cs_a.apply(&mut client_a_doc);
        let (msg_a, k_a, y_a) = client_a_session.push_local(cs_a);

        // 2. Client B deletes 'D' (pos 3)
        let cs_b = cs_del(5, 3, 4);
        cs_b.apply(&mut client_b_doc);
        let (msg_b, k_b, y_b) = client_b_session.push_local(cs_b);

        // 3. Server processes A
        let transformed_a = server_session_a.push_remote(msg_a, k_a, y_a);
        transformed_a.apply(&mut server_doc);
        let (echo_a, k_s_a, y_s_a) = server_session_b.push_local(transformed_a);

        // 4. Server processes B
        let transformed_b = server_session_b.push_remote(msg_b, k_b, y_b);
        transformed_b.apply(&mut server_doc);
        let (echo_b, k_s_b, y_s_b) = server_session_a.push_local(transformed_b);

        // 5. Client A receives Echo B
        let final_b = client_a_session.push_remote(echo_b, k_s_b, y_s_b);
        final_b.apply(&mut client_a_doc);

        // 6. Client B receives Echo A
        let final_a = client_b_session.push_remote(echo_a, k_s_a, y_s_a);
        final_a.apply(&mut client_b_doc);

        assert_eq!(server_doc.to_string(), "ABXCE");
        assert_eq!(client_a_doc.to_string(), server_doc.to_string());
        assert_eq!(client_b_doc.to_string(), server_doc.to_string());
    }

    // ── Paper example (§5, Figure 3) ─────────────────────────────────────────

    #[test]
    fn paper_example_del_d_del_b() {
        let result = converges("ABCDE", cs_del(5, 3, 4), cs_del(5, 1, 2));
        assert_eq!(result, "ACE");
    }

    // ── No overlap ──────────────────────────────────────────────────────────

    #[test]
    fn no_overlap_c_before_s_delete() {
        let r = converges("ABCDE", cs_del(5, 0, 2), cs_del(5, 3, 5));
        assert_eq!(r, "C");
    }

    #[test]
    fn no_overlap_s_before_c_delete() {
        let r = converges("ABCDE", cs_del(5, 3, 5), cs_del(5, 0, 2));
        assert_eq!(r, "C");
    }

    #[test]
    fn no_overlap_c_before_s_insert() {
        let r = converges("ABCD", cs_ins(4, 2, "XY"), cs_ins(4, 4, "Z"));
        assert_eq!(r, "ABXYCDZ");
    }

    #[test]
    fn adjacent_replace_c_before_s() {
        let r = converges("ABCDE", cs(5, 0, 2, "XY"), cs(5, 2, 4, "Z"));
        assert_eq!(r, "XYZE");
    }

    // ── Equal regions ────────────────────────────────────────────────────────

    #[test]
    fn equal_both_pure_delete() {
        let r = converges("ABCDE", cs_del(5, 1, 3), cs_del(5, 1, 3));
        assert_eq!(r, "ADE");
    }

    #[test]
    fn equal_replace_server_wins_ordering() {
        let r = converges("ABCDE", cs(5, 1, 3, "XY"), cs(5, 1, 3, "Z"));
        assert_eq!(r, "AZXYDE");
    }

    #[test]
    fn equal_pure_insert_server_first() {
        let r = converges("AB", cs_ins(2, 1, "XY"), cs_ins(2, 1, "Z"));
        assert_eq!(r, "AZXYB");
    }

    // ── Containment cases ────────────────────────────────────────────────────

    #[test]
    fn c_contains_s_same_ends() {
        let r = converges("ABCDE", cs(5, 0, 4, "W"), cs(5, 1, 3, "Z"));
        assert_eq!(r, "ZWE");
    }

    #[test]
    fn c_contains_s_tail_remains() {
        let r = converges("ABCDE", cs(5, 0, 5, "W"), cs(5, 1, 3, "Z"));
        assert_eq!(r, "ZW");
    }

    #[test]
    fn same_start_c_extends() {
        let r = converges("ABCDE", cs(5, 1, 4, "XY"), cs(5, 1, 3, "Z"));
        assert_eq!(r, "AZXYE");
    }

    #[test]
    fn s_contains_c_same_ends() {
        let r = converges("ABCDE", cs(5, 1, 3, "Z"), cs(5, 0, 4, "W"));
        assert_eq!(r, "ZWE");
    }

    #[test]
    fn s_contains_c_tail_remains() {
        let r = converges("ABCDE", cs(5, 1, 3, "Z"), cs(5, 0, 5, "W"));
        assert_eq!(r, "ZW");
    }

    #[test]
    fn same_start_s_extends() {
        let r = converges("ABCDE", cs(5, 1, 3, "Z"), cs(5, 1, 4, "XY"));
        assert_eq!(r, "AZXYE");
    }

    // ── Overlap cases ────────────────────────────────────────────────────────

    #[test]
    fn c_overlaps_s_left() {
        let r = converges("ABCDE", cs(5, 1, 3, "XY"), cs(5, 2, 4, "Z"));
        assert_eq!(r, "AXYZE");
    }

    #[test]
    fn c_overlaps_s_left_deletions_only() {
        let r = converges("ABCDE", cs_del(5, 0, 3), cs_del(5, 2, 5));
        assert_eq!(r, "");
    }

    #[test]
    fn s_overlaps_c_left() {
        let r = converges("ABCDE", cs(5, 2, 4, "Z"), cs(5, 1, 3, "XY"));
        assert_eq!(r, "AXYZE");
    }

    #[test]
    fn s_overlaps_c_left_deletions_only() {
        let r = converges("ABCDE", cs_del(5, 2, 5), cs_del(5, 0, 3));
        assert_eq!(r, "");
    }

    // ── Additional tests ─────────────────────────────────────────────────────

    #[test]
    fn insert_and_delete_overlap() {
        converges("hello world", cs_ins(11, 5, "!!!"), cs_del(11, 3, 8));
    }

    #[test]
    fn large_replace_vs_small_replace() {
        converges(
            "The quick brown fox",
            cs(19, 4, 15, "slow"),
            cs(19, 10, 15, "red"),
        );
    }

    #[test]
    fn both_pure_inserts_different_positions() {
        let r = converges("AC", cs_ins(2, 1, "B"), cs_ins(2, 2, "D"));
        assert_eq!(r, "ABCD");
    }

    #[test]
    fn idempotent_on_disjoint_deletes_full_string() {
        let r = converges("ABCDEF", cs_del(6, 0, 3), cs_del(6, 3, 6));
        assert_eq!(r, "");
    }

    #[test]
    fn empty_document_both_insert() {
        let r = converges("", cs_ins(0, 0, "hello"), cs_ins(0, 0, "world"));
        assert_eq!(r, "worldhello");
    }

    // ── Jupiter multi-edit test ──────────────────────────────────────────────

    #[test]
    fn test_jupiter_multiple_edits() {
        let mut server_doc = ropey::Rope::from_str("ABCDE");
        let mut client_a_doc = ropey::Rope::from_str("ABCDE");
        let mut client_b_doc = ropey::Rope::from_str("ABCDE");

        let mut server_session_a = JupiterSession::new_server();
        let mut server_session_b = JupiterSession::new_server();
        let mut client_a_session = JupiterSession::new();
        let mut client_b_session = JupiterSession::new();

        // 1. Client A replaces "BCD" (1..4) with "X"
        let cs_a = cs(5, 1, 4, "X");
        cs_a.apply(&mut client_a_doc);
        let (msg_a, k_a, y_a) = client_a_session.push_local(cs_a);

        // 2. Client B inserts "Y" at 2
        let cs_b = cs_ins(5, 2, "Y");
        cs_b.apply(&mut client_b_doc);
        let (msg_b, k_b, y_b) = client_b_session.push_local(cs_b);

        // 3. Server processes A
        let transformed_a = server_session_a.push_remote(msg_a, k_a, y_a);
        transformed_a.apply(&mut server_doc);
        let (echo_a, k_s_a, y_s_a) = server_session_b.push_local(transformed_a);

        // 4. Server processes B
        let transformed_b = server_session_b.push_remote(msg_b, k_b, y_b);
        transformed_b.apply(&mut server_doc);
        let (echo_b, k_s_b, y_s_b) = server_session_a.push_local(transformed_b);

        // 5. Client A receives Echo B
        let final_b = client_a_session.push_remote(echo_b, k_s_b, y_s_b);
        final_b.apply(&mut client_a_doc);

        // 6. Client B receives Echo A
        let final_a = client_b_session.push_remote(echo_a, k_s_a, y_s_a);
        final_a.apply(&mut client_b_doc);

        assert_eq!(server_doc.to_string(), "AYXE");
        assert_eq!(
            client_a_doc.to_string(),
            server_doc.to_string(),
            "Client A diverged"
        );
        assert_eq!(
            client_b_doc.to_string(),
            server_doc.to_string(),
            "Client B diverged"
        );
    }

    // ── ChangeSet basics ─────────────────────────────────────────────────────

    #[test]
    fn changeset_insert() {
        let c = cs_ins(5, 2, "XY");
        assert_eq!(c.in_len, 5);
        assert_eq!(c.out_len, 7);
        let mut doc = ropey::Rope::from_str("ABCDE");
        c.apply(&mut doc);
        assert_eq!(doc.to_string(), "ABXYCDE");
    }

    #[test]
    fn changeset_delete() {
        let c = cs_del(5, 1, 3);
        assert_eq!(c.in_len, 5);
        assert_eq!(c.out_len, 3);
        let mut doc = ropey::Rope::from_str("ABCDE");
        c.apply(&mut doc);
        assert_eq!(doc.to_string(), "ADE");
    }

    #[test]
    fn changeset_replace() {
        let c = cs(5, 1, 3, "XYZ");
        assert_eq!(c.in_len, 5);
        assert_eq!(c.out_len, 6);
        let mut doc = ropey::Rope::from_str("ABCDE");
        c.apply(&mut doc);
        assert_eq!(doc.to_string(), "AXYZDE");
    }

    #[test]
    fn changeset_identity() {
        let c = ChangeSet::identity(5);
        assert_eq!(c.in_len, 5);
        assert_eq!(c.out_len, 5);
        let mut doc = ropey::Rope::from_str("ABCDE");
        c.apply(&mut doc);
        assert_eq!(doc.to_string(), "ABCDE");
    }

    #[test]
    fn changeset_map_pos() {
        // Insert "XY" at position 2 in "ABCDE"
        let c = cs_ins(5, 2, "XY");
        assert_eq!(c.map_pos(0), 0);
        assert_eq!(c.map_pos(1), 1);
        assert_eq!(c.map_pos(2), 2); // at insert point → before (before bias)
        assert_eq!(c.map_pos(3), 5); // after insert, shifted by 2
        assert_eq!(c.map_pos(4), 6);
    }

    #[test]
    fn changeset_invert() {
        let original = ropey::Rope::from_str("ABCDE");
        let c = cs(5, 1, 3, "XYZ");
        let inv = c.invert(&original);

        let mut doc = original.clone();
        c.apply(&mut doc);
        assert_eq!(doc.to_string(), "AXYZDE");
        inv.apply(&mut doc);
        assert_eq!(doc.to_string(), "ABCDE");
    }

    // ── ChangeBuilder ────────────────────────────────────────────────────────

    #[test]
    fn change_builder_multi_edit() {
        // Delete char at 1 ("B"), then insert "X" at pos 3 (original coords)
        let mut b = ChangeBuilder::new(5); // "ABCDE"
        b.advance_to(1);
        b.delete(1); // delete "B"
        b.advance_to(3); // skip "C"
        b.insert("X"); // insert "X" before "D"
        let c = b.finish();
        let mut doc = ropey::Rope::from_str("ABCDE");
        c.apply(&mut doc);
        assert_eq!(doc.to_string(), "ACXDE");
    }

    #[test]
    fn change_builder_out_pos() {
        let mut b = ChangeBuilder::new(5);
        assert_eq!(b.out_pos(), 0);
        b.advance_to(2);
        assert_eq!(b.out_pos(), 2);
        b.delete(1);
        assert_eq!(b.out_pos(), 2); // delete doesn't advance output
        b.insert("XY");
        assert_eq!(b.out_pos(), 4); // inserted 2 chars
    }
}
