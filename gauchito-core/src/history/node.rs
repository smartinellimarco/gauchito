use std::time::Instant;

use crate::edit::Edit;
use crate::selection::SelectionGroup;

pub type NodeId = usize;

#[derive(Clone, Debug)]
pub struct HistoryNode {
    pub edits: Vec<Edit>,
    pub replaced: Vec<String>,
    pub selection_before: Option<SelectionGroup>,
    pub timestamp: Instant,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

impl HistoryNode {
    pub fn new(
        edits: Vec<Edit>,
        replaced: Vec<String>,
        selection_before: Option<SelectionGroup>,
    ) -> Self {
        Self {
            edits,
            replaced,
            selection_before,
            timestamp: Instant::now(),
            parent: None,
            children: Vec::new(),
        }
    }

    /// Inverse edits for undo
    pub fn inverse(&self) -> Vec<Edit> {
        self.edits
            .iter()
            .zip(self.replaced.iter())
            .rev()
            .map(|(edit, replaced)| {
                Edit::new(edit.start, edit.start + edit.text.len(), replaced.clone())
            })
            .collect()
    }
}
