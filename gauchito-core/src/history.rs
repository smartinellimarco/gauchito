use crate::atom::Atom;

#[derive(Clone)]
pub struct SelectionSnapshot {
    pub ranges: Vec<(usize, usize)>,
    pub primary: usize,
}

#[derive(Clone)]
pub struct Transaction {
    pub atoms: Vec<Atom>,
    pub inverses: Vec<Atom>,
    pub selection_before: Option<SelectionSnapshot>,
    pub selection_after: Option<SelectionSnapshot>,
}

impl Transaction {
    pub fn empty() -> Self {
        Transaction {
            atoms: Vec::new(),
            inverses: Vec::new(),
            selection_before: None,
            selection_after: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
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
                txn: Transaction::empty(),
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

    pub fn undo(&mut self) -> Option<Transaction> {
        if self.at_root() {
            return None;
        }

        let revision = &self.revisions[self.current];
        let txn = revision.txn.clone();

        self.current = revision.parent;

        Some(txn)
    }

    pub fn redo(&mut self) -> Option<Transaction> {
        let child = self.revisions[self.current].last?;

        self.current = child;

        Some(self.revisions[self.current].txn.clone())
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
