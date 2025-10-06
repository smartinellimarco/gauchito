use crate::buffer::buffer::Buffer;
use crate::buffer::edit::Edit;
use crate::buffer::history::History;
use crate::buffer::selection::Selection;
use std::path::Path;

#[derive(Debug)]
pub struct Context {
    buffer: Buffer,
    selection: Selection,
    history: History,
}

impl Context {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            selection: Selection::new(0, 0),
            history: History::new(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        Self {
            buffer: Buffer::from_path(path),
            selection: Selection::new(0, 0),
            history: History::new(),
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn history_mut(&mut self) -> &mut History {
        &mut self.history
    }

    // TODO: finish
    // Apply edits updating:
    // - history
    // - tree-sitter
    // - selection
    pub fn apply_edits(&mut self, edits: Vec<Edit>) {
        if edits.is_empty() {
            return;
        }

        // Apply edits to buffer and get deleted texts
        let deleted_texts = self.buffer.apply_edits(&edits);

        // Record in history
        self.history.record(edits.clone(), deleted_texts);

        // Update cursor position
        let new_pos = self.selection.cursor_after_edits(&edits);
        self.selection.cursor_to(new_pos);
    }

    /// Apply single edit (convenience method)
    pub fn apply_edit(&mut self, edit: Edit) {
        self.apply_edits(vec![edit]);
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
