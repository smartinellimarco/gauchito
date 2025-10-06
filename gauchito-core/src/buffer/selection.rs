use crate::buffer::edit::Edit;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    pub fn cursor(position: usize) -> Self {
        Self {
            anchor: position,
            head: position,
        }
    }

    pub fn is_cursor(&self) -> bool {
        self.anchor == self.head
    }

    pub fn cursor_to(&mut self, position: usize) {
        self.anchor = position;
        self.head = position;
    }

    pub fn set_range(&mut self, anchor: usize, head: usize) {
        self.anchor = anchor;
        self.head = head;
    }

    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Calculate cursor position after applying a single edit
    pub fn cursor_after_edit(&self, edit: &Edit) -> usize {
        if edit.is_insert() {
            edit.start + edit.text.len()
        } else if edit.is_delete() {
            edit.start
        } else if edit.is_replace() {
            edit.start + edit.text.len()
        } else {
            // noop
            self.head
        }
    }

    /// Calculate cursor position after applying multiple edits
    pub fn cursor_after_edits(&self, edits: &[Edit]) -> usize {
        if edits.is_empty() {
            return self.head;
        }

        // For multiple edits, position cursor at the end of the last edit
        let last_edit = edits.last().unwrap();
        if last_edit.is_delete() {
            last_edit.start
        } else if last_edit.is_insert() || last_edit.is_replace() {
            last_edit.start + last_edit.text.len()
        } else {
            self.head
        }
    }

    /// Update selection after edits are applied to maintain relative position
    /// This is useful for operations that don't want to move the cursor
    pub fn update_after_edits(&mut self, edits: &[Edit]) {
        for edit in edits {
            self.anchor = Self::adjust_position(self.anchor, edit);
            self.head = Self::adjust_position(self.head, edit);
        }
    }

    fn adjust_position(pos: usize, edit: &Edit) -> usize {
        if pos <= edit.start {
            // Position is before the edit
            pos
        } else if edit.is_insert() {
            // Position is after insertion point
            pos + edit.text.len()
        } else if edit.is_delete() {
            // Position is after deletion
            if pos >= edit.end {
                pos - (edit.end - edit.start)
            } else {
                // Position was inside deleted range
                edit.start
            }
        } else if edit.is_replace() {
            // Position is after replacement
            if pos >= edit.end {
                pos - (edit.end - edit.start) + edit.text.len()
            } else {
                // Position was inside replaced range
                edit.start + edit.text.len()
            }
        } else {
            pos
        }
    }
}
