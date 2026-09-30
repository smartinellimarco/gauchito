//! Buffer — the editable, persistent thing.
//!
//! Named after the vim/emacs convention rather than "Document"
//! because that's what existing users already mean by it. Carries
//! everything that follows edits: pins (for selection endpoints),
//! options, path, revision.
//!
//! [`Buffer::apply`] is the only path that mutates the rope —
//! [`Splice::apply`] is `pub(crate)` so the "apply ⇒ pins project"
//! invariant can't be bypassed. Each call bumps `revision` and sets
//! `modified = true`; `io::write` clears it. `modified` is
//! auto-managed and exposed getter-only on the Lua side — distros
//! never forge it.
//!
//! Pins live on the buffer (not on a sidecar registry) because a pin
//! is meaningless without the rope it indexes into: same buffer,
//! same coordinate space, same lifetime.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ropey::Rope;

use crate::pins::PinTable;
use crate::splice::Splice;

static NEXT_BUFFER_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(pub usize);

impl BufferId {
    pub fn next() -> Self {
        BufferId(NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed))
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
pub struct BufferOptions {
    pub line_ending: LineEnding,
    pub final_newline: bool,
    pub bom: bool,
    pub trim_trailing_whitespace: bool,
    pub indent: IndentStyle,
}

impl Default for BufferOptions {
    fn default() -> Self {
        BufferOptions {
            line_ending: LineEnding::Lf,
            final_newline: true,
            bom: false,
            trim_trailing_whitespace: true,
            indent: IndentStyle::default(),
        }
    }
}

pub struct Buffer {
    pub id: BufferId,
    pub text: Rope,
    pub pins: PinTable, // TODO: porque aca?
    pub options: BufferOptions,
    pub revision: u64,
    pub modified: bool,
    path: Option<PathBuf>,
}

impl Buffer {
    pub fn new(text: Rope, path: Option<PathBuf>, options: BufferOptions) -> Self {
        Buffer {
            id: BufferId::next(),
            text,
            pins: PinTable::new(),
            path,
            options,
            revision: 0,
            modified: false,
        }
    }

    pub fn scratch() -> Self {
        Self::new(Rope::new(), None, BufferOptions::default())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn apply(&mut self, splice: &Splice) -> Splice {
        let inverse = splice.apply(&mut self.text);

        self.pins.apply(splice);
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
