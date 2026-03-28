use crate::selection::Selection;
use gauchito_ot::ChangeSet;

// ── Revision ────────────────────────────────────────────────────────────────

/// One node in the history tree.
struct Revision {
    parent: usize,
    last_child: Option<usize>,
    forward: ChangeSet,
    inverse: ChangeSet,
    selection: Selection,
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
                selection: Selection::point(0),
            }],
            current: 0,
        }
    }

    /// Record a new revision as a child of the current one.
    pub fn commit(&mut self, forward: ChangeSet, inverse: ChangeSet, selection: Selection) {
        let new_idx = self.revisions.len();

        // Point the current revision's redo branch at the new one
        self.revisions[self.current].last_child = Some(new_idx);

        self.revisions.push(Revision {
            parent: self.current,
            last_child: None,
            forward,
            inverse,
            selection,
        });

        self.current = new_idx;
    }

    /// Walk one step toward the root.
    /// Returns the inverse changeset and the selection to restore.
    pub fn undo(&mut self) -> Option<(ChangeSet, Selection)> {
        if self.at_root() {
            return None;
        }

        let revision = &self.revisions[self.current];
        let inverse = revision.inverse.clone();
        let selection = revision.selection.clone();

        self.current = revision.parent;

        Some((inverse, selection))
    }

    /// Follow the most recent child branch one step.
    /// Returns the forward changeset to re-apply.
    pub fn redo(&mut self) -> Option<ChangeSet> {
        let child = self.revisions[self.current].last_child?;

        self.current = child;

        Some(self.revisions[self.current].forward.clone())
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
    use gauchito_ot::ChangeBuilder;
    use ropey::Rope;

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
        h.commit(cs, inv, Selection::point(0));

        let mut rope = Rope::from_str("hello world");
        let (inv, _selection) = h.undo().unwrap();
        inv.apply(&mut rope);
        assert_eq!(rope.to_string(), "hello");

        let fwd = h.redo().unwrap();
        fwd.apply(&mut rope);
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn branch_after_undo() {
        let mut h = History::new();
        let sel = Selection::point(0);

        let r0 = Rope::from_str("a");
        let mut b = ChangeBuilder::new(1);
        b.advance_to(1);
        b.insert("b");
        let cs1 = b.finish();
        let inv1 = cs1.invert(&r0);
        h.commit(cs1, inv1, sel.clone()); // "ab"

        h.undo().unwrap(); // back to "a"

        let r1 = Rope::from_str("a");
        let mut b = ChangeBuilder::new(1);
        b.advance_to(1);
        b.insert("c");
        let cs2 = b.finish();
        let inv2 = cs2.invert(&r1);
        h.commit(cs2, inv2, sel); // "ac" — new branch

        h.undo().unwrap();
        let fwd = h.redo().unwrap();
        let mut rope = Rope::from_str("a");
        fwd.apply(&mut rope);
        assert_eq!(rope.to_string(), "ac"); // follows newest branch
    }
}
