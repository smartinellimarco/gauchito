use std::path::PathBuf;

use ropey::Rope;

use crate::anchor::AnchorTable;
use crate::history::{History, SelectionSnapshot, Transaction};
pub use crate::ids::{DocumentId, ViewId};
use crate::mutation::Mutation;
use crate::options::{DocumentOptions, PartialDocumentOptions};

pub struct Document {
    pub id: DocumentId,
    pub text: Rope,
    pub anchors: AnchorTable,
    pub path: Option<PathBuf>,
    pub options: DocumentOptions,
    pub revision: u64,
    history: History,
}

impl Document {
    pub fn new() -> Self {
        Self::from_rope(Rope::new(), None)
    }

    pub fn from_rope(text: Rope, options: Option<DocumentOptions>) -> Self {
        Document {
            id: DocumentId::next(),
            text,
            anchors: AnchorTable::new(),
            path: None,
            options: options.unwrap_or_default(),
            revision: 0,
            history: History::new(),
        }
    }

    pub fn apply_options(&mut self, partial: PartialDocumentOptions) {
        let current = std::mem::take(&mut self.options);

        self.options = partial.resolve(current);
    }

    pub fn commit(&mut self, transaction: Transaction) {
        self.history.commit(transaction);
    }

    pub fn undo(&mut self) -> Option<(Vec<Mutation>, Option<SelectionSnapshot>)> {
        let (inverses, snap) = self.history.undo()?;

        for atom in inverses.iter().rev() {
            atom.apply(&mut self.text);
            self.anchors.apply_atom(atom);
            self.revision += 1;
        }

        Some((inverses, snap))
    }

    pub fn redo(&mut self) -> Option<(Vec<Mutation>, Option<SelectionSnapshot>)> {
        let (atoms, snap) = self.history.redo()?;

        for atom in &atoms {
            atom.apply(&mut self.text);
            self.anchors.apply_atom(atom);
            self.revision += 1;
        }

        Some((atoms, snap))
    }

    pub fn name(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("scratch")
    }
}
