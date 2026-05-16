use crate::mutation::Mutation;

#[derive(Clone)]
pub struct SelectionSnapshot {
    pub ranges: Vec<(usize, usize)>,
    pub primary: usize,
}

pub struct Transaction {
    pub mutations: Vec<Mutation>,
    pub inverses: Vec<Mutation>,
    pub selection_before: Option<SelectionSnapshot>,
    pub selection_after: Option<SelectionSnapshot>,
}

struct Revision {
    parent: usize,
    last: Option<usize>,
    txn: Transaction,
}

pub struct History {
    revisions: Vec<Revision>,
    current: usize,
}

impl History {
    pub fn new() -> Self {
        History {
            revisions: vec![Revision {
                parent: 0,
                last: None,
                txn: Transaction {
                    mutations: Vec::new(),
                    inverses: Vec::new(),
                    selection_before: None,
                    selection_after: None,
                },
            }],
            current: 0,
        }
    }

    pub fn commit(&mut self, txn: Transaction) {
        let id = self.revisions.len();

        self.revisions[self.current].last = Some(id);

        self.revisions.push(Revision {
            parent: self.current,
            last: None,
            txn,
        });

        self.current = id;
    }

    pub fn undo(&mut self) -> Option<(Vec<Mutation>, Option<SelectionSnapshot>)> {
        if self.at_root() {
            return None;
        }

        let revision = &self.revisions[self.current];
        let inverses = revision.txn.inverses.clone();
        let selection = revision.txn.selection_before.clone();

        self.current = revision.parent;

        Some((inverses, selection))
    }

    pub fn redo(&mut self) -> Option<(Vec<Mutation>, Option<SelectionSnapshot>)> {
        let child = self.revisions[self.current].last?;

        self.current = child;

        let revision = &self.revisions[self.current];

        Some((
            revision.txn.mutations.clone(),
            revision.txn.selection_after.clone(),
        ))
    }

    pub fn at_root(&self) -> bool {
        self.current == 0
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    fn snap(pos: usize) -> SelectionSnapshot {
        SelectionSnapshot {
            ranges: vec![(pos, pos)],
            primary: 0,
        }
    }

    fn apply_forward(rope: &mut Rope, mutations: &[Mutation]) {
        for a in mutations {
            a.apply(rope);
        }
    }

    #[test]
    fn linear_undo_redo() {
        let mut h = History::new();
        let fwd = vec![Mutation::new(5, 5, " world".to_string())];

        let mut rope = Rope::from_str("hello");
        let inverses: Vec<Mutation> = fwd.iter().map(|m| m.apply(&mut rope)).collect();
        assert_eq!(rope.to_string(), "hello world");

        let txn = Transaction {
            mutations: fwd.clone(),
            inverses: inverses.clone(),
            selection_before: Some(snap(0)),
            selection_after: Some(snap(11)),
        };
        h.commit(txn);

        // undo returns the stored inverses; applying them in reverse reverts.
        let (undo_atoms, _) = h.undo().unwrap();
        assert_eq!(undo_atoms.len(), 1);
        for atom in undo_atoms.iter().rev() {
            atom.apply(&mut rope);
        }
        assert_eq!(rope.to_string(), "hello");

        // redo returns the forwards; re-applying brings the buffer back.
        let (redo_atoms, _) = h.redo().unwrap();
        apply_forward(&mut rope, &redo_atoms);
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn branch_after_undo() {
        let mut h = History::new();

        let f1 = vec![Mutation::new(1, 1, "b".to_string())];
        let txn1 = Transaction {
            mutations: f1,
            inverses: Vec::new(),
            selection_before: Some(snap(0)),
            selection_after: None,
        };
        h.commit(txn1);

        h.undo().unwrap();

        let f2 = vec![Mutation::new(1, 1, "c".to_string())];
        let txn2 = Transaction {
            mutations: f2,
            inverses: Vec::new(),
            selection_before: Some(snap(0)),
            selection_after: None,
        };
        h.commit(txn2);

        h.undo().unwrap();
        let (mutations, _) = h.redo().unwrap();
        let mut rope = Rope::from_str("a");
        apply_forward(&mut rope, &mutations);
        assert_eq!(rope.to_string(), "ac");
    }

    #[test]
    fn multi_atom_transaction_roundtrips() {
        let mut h = History::new();
        let mutations = vec![
            Mutation::new(0, 0, "a".to_string()),
            Mutation::new(1, 1, "b".to_string()),
        ];

        let mut rope = Rope::from_str("");
        let inverses: Vec<Mutation> = mutations.iter().map(|m| m.apply(&mut rope)).collect();
        assert_eq!(rope.to_string(), "ab");

        let txn = Transaction {
            mutations: mutations.clone(),
            inverses,
            selection_before: Some(snap(0)),
            selection_after: None,
        };
        h.commit(txn);

        let (undo_atoms, _) = h.undo().unwrap();
        assert_eq!(undo_atoms.len(), 2);

        for inv in undo_atoms.iter().rev() {
            inv.apply(&mut rope);
        }
        assert_eq!(rope.to_string(), "");
    }
}
