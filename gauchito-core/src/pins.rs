//! Pins — durable offsets into a buffer, projected through edits.
//!
//! Pins are kept in offset-sorted order so the per-splice apply path
//! only touches pins that can actually move. A splice at position p
//! cannot affect any pin with offset < p (φ's first branch returns
//! the offset unchanged), so we binary-search to the first pin at-or-
//! after p and walk only the suffix.
//!
//! φ is not strictly monotonic: two pins with offsets in [p,q] but
//! opposite gravity swap order after the splice collapses them to
//! s.p (Left) vs s.p+n (Right). So apply can't just mutate offsets in
//! place — the [p,q] section of the suffix has to be re-bucketed by
//! gravity. Pins originally past s.q shift uniformly and keep order,
//! so they ride along untouched.
//!
//! PERF: worst case is still O(P) (edit at offset 0 moves every
//! pin), which is unavoidable. The win is typical-case: an edit near
//! the bottom of a file only touches a small suffix, and multi-cursor
//! edits in disjoint regions stop re-walking the whole table.
//!
//! `pins_in(from, to)` is a flat offset-range query — two binary
//! searches and a slice. It's the primitive a viewport decoration
//! query can sit on. Decorations longer than the window (folds,
//! virtual lines) are not handled by this shape; that workload is
//! what would justify an interval tree, and it's intentionally out
//! of scope until those decoration kinds ship.

use std::collections::HashMap;

use crate::splice::{Gravity, Splice, phi};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PinId(usize);

#[derive(Clone, Copy, Debug)]
struct Entry {
    offset: usize,
    gravity: Gravity,
}

#[derive(Clone, Default)]
pub struct PinTable {
    next_id: usize,
    entries: HashMap<PinId, Entry>,
    // Pin ids sorted by offset. HashMap is the source of truth for
    // entry data; this Vec is just an ordering index. Internal order
    // within an equal-offset bucket is unspecified — callers must not
    // rely on it.
    order: Vec<PinId>,
}

impl PinTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, offset: usize, gravity: Gravity) -> PinId {
        let id = PinId(self.next_id);
        self.next_id += 1;

        let entries = &self.entries;
        let pos = self.order.partition_point(|other| entries[other].offset <= offset);

        self.entries.insert(id, Entry { offset, gravity });
        self.order.insert(pos, id);
        id
    }

    pub fn add_with_gravity(&mut self, offset: usize, gravity: Gravity) -> PinId {
        self.add(offset, gravity)
    }

    pub fn remove(&mut self, id: PinId) {
        let Some(entry) = self.entries.get(&id).copied() else { return };
        if let Some(pos) = self.locate(id, entry) {
            self.order.remove(pos);
        }
        self.entries.remove(&id);
    }

    pub fn offset(&self, id: PinId) -> usize {
        self.entries[&id].offset
    }

    pub fn gravity(&self, id: PinId) -> Gravity {
        self.entries[&id].gravity
    }

    pub fn set_offset(&mut self, id: PinId, offset: usize) {
        let Some(old) = self.entries.get(&id).copied() else { return };
        if old.offset == offset {
            return;
        }
        self.reindex(id, old, Entry { offset, gravity: old.gravity });
    }

    pub fn set_gravity(&mut self, id: PinId, gravity: Gravity) {
        let Some(old) = self.entries.get(&id).copied() else { return };
        if old.gravity == gravity {
            return;
        }
        self.reindex(id, old, Entry { offset: old.offset, gravity });
    }

    pub fn apply(&mut self, s: &Splice) {
        let entries = &self.entries;
        let suffix_start = self.order.partition_point(|id| entries[id].offset < s.p());

        // Walk the suffix, applying φ in place. Pins originally in
        // [s.p, s.q] collapse to either s.p or s.p+n, crossing each
        // other — so segregate them by where they land. Pins
        // originally > s.q shift uniformly; φ preserves their relative
        // order, so they go into `tail` as-is.
        let mut left_collapse = Vec::new();
        let mut right_collapse = Vec::new();
        let mut tail = Vec::new();

        for i in suffix_start..self.order.len() {
            let id = self.order[i];
            let entry = self
                .entries
                .get_mut(&id)
                .expect("order mirrors entries");
            let was = entry.offset;
            entry.offset = phi(s, was, entry.gravity);

            if was <= s.q() {
                if entry.offset == s.p() {
                    left_collapse.push(id);
                } else {
                    right_collapse.push(id);
                }
            } else {
                tail.push(id);
            }
        }

        self.order.truncate(suffix_start);
        self.order.extend(left_collapse);
        self.order.extend(right_collapse);
        self.order.extend(tail);
    }

    /// Pin ids whose offset lies in `[from, to)`, in index order.
    pub fn pins_in(&self, from: usize, to: usize) -> impl Iterator<Item = PinId> + '_ {
        let entries = &self.entries;
        let start = self.order.partition_point(|id| entries[id].offset < from);
        let end = self.order.partition_point(|id| entries[id].offset < to);
        self.order[start..end].iter().copied()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find `id` in `order`. Binary-searches to the offset bucket
    /// then linearly scans for the exact id. Buckets are tiny
    /// in practice (usually 1; pin-per-cursor, pin-per-decoration-end).
    fn locate(&self, id: PinId, entry: Entry) -> Option<usize> {
        let entries = &self.entries;
        let start = self.order.partition_point(|other| entries[other].offset < entry.offset);
        for at in start..self.order.len() {
            let other = self.order[at];
            if other == id {
                return Some(at);
            }
            if entries[&other].offset != entry.offset {
                return None;
            }
        }
        None
    }

    fn reindex(&mut self, id: PinId, old: Entry, new: Entry) {
        if let Some(pos) = self.locate(id, old) {
            self.order.remove(pos);
        }
        let slot = self.entries.get_mut(&id).expect("entry was present a moment ago");
        *slot = new;

        let entries = &self.entries;
        let pos = self.order.partition_point(|other| entries[other].offset <= new.offset);
        self.order.insert(pos, id);
    }
}
