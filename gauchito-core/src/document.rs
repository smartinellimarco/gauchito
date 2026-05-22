use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ropey::Rope;

use crate::anchor::AnchorTable;
use crate::atom::Atom;

static NEXT_DOCUMENT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub usize);

impl DocumentId {
    pub fn next() -> Self {
        DocumentId(NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentStyle {
    pub unit: String,
    pub tab_width: u8,
}

impl Default for IndentStyle {
    fn default() -> Self {
        IndentStyle {
            unit: " ".repeat(4).to_string(),
            tab_width: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocumentOptions {
    pub line_ending: LineEnding,
    pub final_newline: bool,
    pub bom: bool,
    pub trim_trailing_whitespace: bool,
    pub indent: IndentStyle,
}

impl Default for DocumentOptions {
    fn default() -> Self {
        DocumentOptions {
            line_ending: LineEnding::Lf,
            final_newline: true,
            bom: false,
            trim_trailing_whitespace: true,
            indent: IndentStyle::default(),
        }
    }
}

pub struct Document {
    pub id: DocumentId,
    pub text: Rope,
    pub anchors: AnchorTable,
    pub options: DocumentOptions,
    pub revision: u64,
    pub modified: bool,
    path: Option<PathBuf>,
}

impl Document {
    pub fn new(text: Rope, path: Option<PathBuf>, options: DocumentOptions) -> Self {
        Document {
            id: DocumentId::next(),
            text,
            anchors: AnchorTable::new(),
            path,
            options,
            revision: 0,
            modified: false,
        }
    }

    pub fn scratch() -> Self {
        Self::new(Rope::new(), None, DocumentOptions::default())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn apply(&mut self, atom: &Atom) -> Atom {
        let inverse = atom.apply(&mut self.text);

        self.anchors.apply(atom);
        self.revision += 1;
        self.modified = true;

        inverse
    }

    pub fn name(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("scratch")
    }
}
