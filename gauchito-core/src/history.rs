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
