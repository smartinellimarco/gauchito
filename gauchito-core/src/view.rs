//! View — a cursor session on a buffer.
//!
//! `{ id, buf, selection }` — the entire substrate concept. No
//! `mode`, no scroll offsets, no `cursor_style`, no `hidden`: all
//! editor policy is distro Lua. Per-view mode/scroll is a Lua table
//! keyed by `view.id`; cursor style is a render-time opt; "hidden"
//! is "the distro chose not to paint it this frame."
//!
//! [`Drop`] releases the selection's pins back to the buffer's pin
//! table — selection pins are scoped to the buffer that issued them,
//! and forgetting to free them would leak `PinTable` entries forever.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::buffer::Buffer;
use crate::selection::Selection;

static NEXT_VIEW_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(pub usize);

impl ViewId {
    pub fn next() -> Self {
        ViewId(NEXT_VIEW_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub struct View {
    pub id:        ViewId,
    pub buf:       Rc<RefCell<Buffer>>,
    pub selection: Selection,
}

impl View {
    pub fn new(buf: Rc<RefCell<Buffer>>) -> Self {
        let selection = Selection::point(&mut buf.borrow_mut().pins, 0);
        View {
            id: ViewId::next(),
            buf,
            selection,
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        if let Ok(mut buf) = self.buf.try_borrow_mut() {
            self.selection.drop(&mut buf.pins);
        }
    }
}
