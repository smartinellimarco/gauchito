use std::time::{Duration, Instant};

use super::node::{HistoryNode, NodeId};
use crate::edit::Edit;
use crate::selection::SelectionGroup;

pub struct HistoryTree {
    nodes: Vec<HistoryNode>,
    current: NodeId,
    merge_threshold: Duration,
}

impl HistoryTree {
    pub fn new(merge_threshold_ms: u64) -> Self {
        let root = HistoryNode::new(Vec::new(), Vec::new(), None);
        Self {
            nodes: vec![root],
            current: 0,
            merge_threshold: Duration::from_millis(merge_threshold_ms),
        }
    }

    pub fn current(&self) -> NodeId {
        self.current
    }

    pub fn node(&self, id: NodeId) -> &HistoryNode {
        &self.nodes[id]
    }

    pub fn record(
        &mut self,
        edits: Vec<Edit>,
        replaced: Vec<String>,
        selection_before: SelectionGroup,
    ) -> NodeId {
        let now = Instant::now();
        let cur = &self.nodes[self.current];

        // try merge into current node
        let merge = !cur.edits.is_empty()
            && cur.children.is_empty()
            && now.duration_since(cur.timestamp) < self.merge_threshold;

        if merge {
            let node = &mut self.nodes[self.current];
            node.edits.extend(edits);
            node.replaced.extend(replaced);
            // keep the original selection_before from the first edit in the group
            node.timestamp = now;
            self.current
        } else {
            let id = self.nodes.len();
            let mut node = HistoryNode::new(edits, replaced, Some(selection_before));
            node.parent = Some(self.current);
            self.nodes[self.current].children.push(id);
            self.nodes.push(node);
            self.current = id;
            id
        }
    }

    pub fn edits(&self, id: NodeId) -> &[Edit] {
        &self.nodes[id].edits
    }

    pub fn undo(&mut self) -> Option<(Vec<Edit>, Option<SelectionGroup>, NodeId)> {
        let cur = &self.nodes[self.current];
        cur.parent.map(|parent_id| {
            let inverse = cur.inverse();
            let sel = cur.selection_before.clone();
            self.current = parent_id;
            (inverse, sel, parent_id)
        })
    }

    pub fn redo(&mut self) -> Option<(Vec<Edit>, Option<SelectionGroup>, NodeId)> {
        let cur = &self.nodes[self.current];
        cur.children.first().map(|&child_id| {
            let child = &self.nodes[child_id];
            let edits = child.edits.clone();
            let sel = child.selection_before.clone();
            self.current = child_id;
            (edits, sel, child_id)
        })
    }
}

impl Default for HistoryTree {
    fn default() -> Self {
        Self::new(500)
    }
}
