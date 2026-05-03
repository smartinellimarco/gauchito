use std::io::{self, Write};
use std::path::PathBuf;

use ropey::Rope;

use crate::anchor::AnchorTable;
use crate::changeset::ChangeSet;
use crate::history::{History, SelectionSnapshot};
pub use crate::ids::{DocumentId, ViewId};
use crate::options::DocumentOptions;

pub struct Document {
    pub id: DocumentId,
    pub text: Rope,
    pub anchors: AnchorTable,
    pub path: Option<PathBuf>,
    pub options: DocumentOptions,

    history: History,
}

impl Document {
    pub fn new() -> Self {
        Self::from_rope(Rope::new())
    }

    /// Build from an existing rope. Auto-allocates a `DocumentId`.
    pub fn from_rope(text: Rope) -> Self {
        Document {
            id: DocumentId::next(),
            text,
            anchors: AnchorTable::new(),
            path: None,
            options: DocumentOptions::default(),
            history: History::new(),
        }
    }

    // ── Mutation ────────────────────────────────────────────────────────

    /// Apply a changeset to the rope and advance every anchor in lockstep.
    /// Returns the inverse changeset for history/rollback. Does not commit
    /// to history — callers decide when to record.
    pub fn apply(&mut self, cs: &ChangeSet) -> ChangeSet {
        let inverse = cs.invert(&self.text);

        cs.apply(&mut self.text);

        self.anchors.apply(cs);

        inverse
    }

    // ── History ─────────────────────────────────────────────────────────

    /// Record a completed edit in the undo history.
    pub fn commit(
        &mut self,
        forward: ChangeSet,
        inverse: ChangeSet,
        selection_before: Option<SelectionSnapshot>,
        selection_after: Option<SelectionSnapshot>,
    ) {
        self.history
            .commit(forward, inverse, selection_before, selection_after);
    }

    /// Undo the last edit. Applies the inverse changeset to the rope and
    /// updates anchors. Returns the inverse changeset and an optional
    /// selection snapshot for the caller to rehydrate into view selections.
    pub fn undo(&mut self) -> Option<(ChangeSet, Option<SelectionSnapshot>)> {
        let (inverse, snap) = self.history.undo()?;

        inverse.apply(&mut self.text);

        self.anchors.apply(&inverse);

        Some((inverse, snap))
    }

    /// Redo the last undone edit. Applies the forward changeset and
    /// updates anchors. Returns the forward changeset and an optional
    /// selection snapshot for the caller to rehydrate.
    pub fn redo(&mut self) -> Option<(ChangeSet, Option<SelectionSnapshot>)> {
        let (forward, snap) = self.history.redo()?;

        forward.apply(&mut self.text);

        self.anchors.apply(&forward);

        Some((forward, snap))
    }

    // ── Queries ─────────────────────────────────────────────────────────

    /// True if the document has unsaved changes.
    pub fn is_modified(&self) -> bool {
        !self.history.at_root()
    }

    /// Replace the document text with new content and reset history.
    pub fn reload(&mut self, text: Rope) {
        self.text = text;
        self.history = History::new();
    }

    /// File name for display, or `[scratch]` if no path is set.
    pub fn name(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[scratch]")
    }

    // ── Save ───────────────────────────────────────────────────────────

    // TODO: move outside to utils -> fileio.rs
    /// Write the document to its file path, applying format options.
    pub fn write(&self) -> io::Result<()> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "document has no file path"))?;

        let file = std::fs::File::create(path)?;
        let mut w = io::BufWriter::new(file);

        if self.options.bom {
            w.write_all("\u{feff}".as_bytes())?;
        }

        let sep = self.options.line_ending.as_str();

        for line in self.text.lines() {
            let mut content: String = line.chars().collect();

            // Strip the trailing \n that ropey keeps on every line except possibly the last.
            let had_newline = content.ends_with('\n');
            if had_newline {
                content.pop();
            }

            if self.options.trim_trailing_whitespace {
                let trimmed = content.trim_end();
                content.truncate(trimmed.len());
            }

            w.write_all(content.as_bytes())?;

            if had_newline {
                w.write_all(sep.as_bytes())?;
            }
        }

        // Ensure a final newline if the option says so and the file doesn't already end with one.
        if self.options.final_newline {
            let len = self.text.len_chars();
            let ends_with_nl = len > 0 && self.text.char(len - 1) == '\n';
            if !ends_with_nl {
                w.write_all(sep.as_bytes())?;
            }
        }

        w.flush()
    }
}
