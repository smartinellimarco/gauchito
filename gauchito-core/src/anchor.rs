//! Stable position handles into a document.
//!
//! An [`AnchorId`] is a document-scoped handle that always resolves to
//! the "same logical position" even as the rope is edited. Holders (selections,
//! marks, LSP diagnostics, diff hunks) store the id; the [`AnchorTable`] keeps
//! the live offset for every id and updates them in lockstep with the rope.
//!
//! When the rope changes, callers feed the [`ChangeSet`] to [`AnchorTable::apply`]
//! and every anchor advances by the same `map_pos` rule that selections used to
//! apply individually. Sibling views, marks, and future LSP positions all track
//! automatically with no per-consumer mapping code.
use std::collections::HashMap;

use crate::changeset::{Bias, ChangeSet};
pub use crate::ids::AnchorId;

#[derive(Debug, Clone)]
pub struct AnchorTable {
    entries: HashMap<AnchorId, (usize, Bias)>,
    next: u64,
}

impl AnchorTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next: 1,
        }
    }

    /// Allocate a new anchor at `offset`. Returns its handle.
    pub fn create(&mut self, offset: usize, bias: Bias) -> AnchorId {
        let id = AnchorId(self.next);

        self.next += 1;
        self.entries.insert(id, (offset, bias));

        id
    }

    /// Free an anchor. Idempotent — dropping an already-dropped id is a no-op.
    pub fn drop(&mut self, id: AnchorId) {
        self.entries.remove(&id);
    }

    /// Resolve an anchor to its current offset.
    ///
    /// # Panics
    /// If `id` was never created or has been dropped.
    pub fn offset(&self, id: AnchorId) -> usize {
        self.entries[&id].0
    }

    /// Look up offset, returning `None` if the anchor is gone.
    pub fn try_offset(&self, id: AnchorId) -> Option<usize> {
        self.entries.get(&id).map(|(o, _)| *o)
    }

    /// Resolve an anchor's bias.
    pub fn bias(&self, id: AnchorId) -> Bias {
        self.entries[&id].1
    }

    /// Move an existing anchor to a new offset. Used by motion kernels so a
    /// single cursor identity persists as it moves around.
    pub fn set_offset(&mut self, id: AnchorId, offset: usize) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.0 = offset;
        }
    }

    /// Walk every anchor through `cs` using its stored bias. Call this exactly
    /// once per rope mutation.
    pub fn apply(&mut self, cs: &ChangeSet) {
        for (offset, bias) in self.entries.values_mut() {
            *offset = cs.map_pos(*offset, *bias);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AnchorTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changeset::ChangeBuilder;
    use ropey::Rope;

    fn insert_at(rope: &Rope, pos: usize, text: &str) -> ChangeSet {
        let mut b = ChangeBuilder::new(rope.len_chars());

        b.advance_to(pos);
        b.insert(text);
        b.finish()
    }

    fn delete_at(rope: &Rope, pos: usize, n: usize) -> ChangeSet {
        let mut b = ChangeBuilder::new(rope.len_chars());

        b.advance_to(pos);
        b.delete(n);
        b.finish()
    }

    #[test]
    fn create_and_resolve() {
        let mut t = AnchorTable::new();
        let a = t.create(5, Bias::After);
        let b = t.create(10, Bias::After);

        assert_eq!(t.offset(a), 5);
        assert_eq!(t.offset(b), 10);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn drop_is_idempotent() {
        let mut t = AnchorTable::new();
        let a = t.create(0, Bias::After);

        t.drop(a);
        t.drop(a);

        assert!(t.try_offset(a).is_none());
    }

    #[test]
    fn set_offset_mutates_in_place() {
        let mut t = AnchorTable::new();
        let a = t.create(5, Bias::After);

        t.set_offset(a, 42);

        assert_eq!(t.offset(a), 42);
    }

    #[test]
    fn apply_shifts_anchors_after_insert() {
        let rope: Rope = "hello world".into();
        let mut t = AnchorTable::new();
        let before = t.create(2, Bias::After);
        let after = t.create(8, Bias::After);

        // Insert "XX" at position 5.
        let cs = insert_at(&rope, 5, "XX");

        t.apply(&cs);

        assert_eq!(t.offset(before), 2);
        assert_eq!(t.offset(after), 10);
    }

    #[test]
    fn apply_shifts_anchors_after_delete() {
        let rope: Rope = "hello world".into();
        let mut t = AnchorTable::new();
        let inside = t.create(7, Bias::After);
        let outside = t.create(2, Bias::After);

        // Delete 3 chars starting at 5: "hello world" → "helloorld".
        let cs = delete_at(&rope, 5, 3);

        t.apply(&cs);

        assert_eq!(t.offset(outside), 2);
        assert_eq!(t.offset(inside), 5);
    }

    #[test]
    fn inverse_undoes_anchor_movement() {
        let rope: Rope = "hello".into();
        let mut t = AnchorTable::new();
        let id = t.create(3, Bias::After);
        let forward = insert_at(&rope, 1, "ZZ");
        let inverse = forward.invert(&rope);

        t.apply(&forward);

        assert_eq!(t.offset(id), 5);

        // Apply inverse — anchor should return to original offset.
        t.apply(&inverse);

        assert_eq!(t.offset(id), 3);
    }
}
