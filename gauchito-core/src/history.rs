//! Tree-structured undo/redo history.
//!
//! Selections are stored as resolved offset snapshots, not anchors —
//! anchors are document-scoped handles whose ids cannot survive across
//! the lifetime of the history record. On undo/redo, callers rehydrate
//! the snapshot back into fresh anchors in the document's `AnchorTable`.

use crate::changeset::ChangeSet;

// ── SelectionSnapshot ───────────────────────────────────────────────────────

/// Frozen `(anchor, head)` ranges for one selection. Re-allocated to a real
/// `Selection` against an `AnchorTable` on undo/redo.
#[derive(Debug, Clone)]
pub struct SelectionSnapshot {
    pub ranges: Vec<(usize, usize)>,
    pub primary: usize,
}

// ── Revision ────────────────────────────────────────────────────────────────

/// One node in the history tree.
struct Revision {
    parent: usize,
    last_child: Option<usize>,
    forward: ChangeSet,
    inverse: ChangeSet,
    /// Selection to restore on undo. None = map cursor through inverse instead.
    selection_before: Option<SelectionSnapshot>,
    /// Selection to restore on redo. None = map cursor through forward instead.
    selection_after: Option<SelectionSnapshot>,
}

// ── History ─────────────────────────────────────────────────────────────────

/// Tree-structured undo history.
pub struct History {
    revisions: Vec<Revision>,
    current: usize,
}

impl History {
    pub fn new() -> Self {
        // Revision 0 is a sentinel root — it carries no real edit.
        History {
            revisions: vec![Revision {
                parent: 0,
                last_child: None,
                forward: ChangeSet::identity(0),
                inverse: ChangeSet::identity(0),
                selection_before: None,
                selection_after: None,
            }],
            current: 0,
        }
    }

    /// Record a new revision as a child of the current one.
    /// `selection_before` is restored on undo, `selection_after` on redo.
    /// Pass `None` for either to use position mapping instead.
    pub fn commit(
        &mut self,
        forward: ChangeSet,
        inverse: ChangeSet,
        selection_before: Option<SelectionSnapshot>,
        selection_after: Option<SelectionSnapshot>,
    ) {
        let new_idx = self.revisions.len();

        // Point the current revision's redo branch at the new one
        self.revisions[self.current].last_child = Some(new_idx);

        self.revisions.push(Revision {
            parent: self.current,
            last_child: None,
            forward,
            inverse,
            selection_before,
            selection_after,
        });

        self.current = new_idx;
    }

    /// Walk one step toward the root.
    /// Returns the inverse changeset and an optional selection snapshot.
    pub fn undo(&mut self) -> Option<(ChangeSet, Option<SelectionSnapshot>)> {
        if self.at_root() {
            return None;
        }

        let revision = &self.revisions[self.current];
        let inverse = revision.inverse.clone();
        let selection = revision.selection_before.clone();

        self.current = revision.parent;

        Some((inverse, selection))
    }

    /// Follow the most recent child branch one step.
    /// Returns the forward changeset and an optional selection snapshot.
    pub fn redo(&mut self) -> Option<(ChangeSet, Option<SelectionSnapshot>)> {
        let child = self.revisions[self.current].last_child?;

        self.current = child;

        let revision = &self.revisions[self.current];
        Some((revision.forward.clone(), revision.selection_after.clone()))
    }

    /// True when there is nothing to undo.
    pub fn at_root(&self) -> bool {
        self.current == 0
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changeset::ChangeBuilder;
    use ropey::Rope;

    fn snap(pos: usize) -> SelectionSnapshot {
        SelectionSnapshot {
            ranges: vec![(pos, pos)],
            primary: 0,
        }
    }

    #[test]
    fn linear_undo_redo() {
        let mut h = History::new();
        let orig = Rope::from_str("hello");

        // Insert " world" at position 5
        let mut b = ChangeBuilder::new(5);
        b.advance_to(5);
        b.insert(" world");
        let cs = b.finish();
        let inv = cs.invert(&orig);
        h.commit(cs, inv, Some(snap(0)), None);

        let mut rope = Rope::from_str("hello world");
        let (inv, _selection) = h.undo().unwrap();
        inv.apply(&mut rope);
        assert_eq!(rope.to_string(), "hello");

        let (fwd, _) = h.redo().unwrap();
        fwd.apply(&mut rope);
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn branch_after_undo() {
        let mut h = History::new();

        let r0 = Rope::from_str("a");
        let mut b = ChangeBuilder::new(1);
        b.advance_to(1);
        b.insert("b");
        let cs1 = b.finish();
        let inv1 = cs1.invert(&r0);
        h.commit(cs1, inv1, Some(snap(0)), None); // "ab"

        h.undo().unwrap(); // back to "a"

        let r1 = Rope::from_str("a");
        let mut b = ChangeBuilder::new(1);
        b.advance_to(1);
        b.insert("c");
        let cs2 = b.finish();
        let inv2 = cs2.invert(&r1);
        h.commit(cs2, inv2, Some(snap(0)), None); // "ac" — new branch

        h.undo().unwrap();
        let (fwd, _) = h.redo().unwrap();
        let mut rope = Rope::from_str("a");
        fwd.apply(&mut rope);
        assert_eq!(rope.to_string(), "ac"); // follows newest branch
    }
}
