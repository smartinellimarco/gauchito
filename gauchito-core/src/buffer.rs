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

pub struct Buffer {
    pub id: BufferId,
    pub text: Rope,
    pub pins: PinTable, // TODO: porque aca?
    pub modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Buffer {
            id: BufferId::next(),
            text: Rope::new(),
            pins: PinTable::new(),
            modified: false,
        }
    }

    pub fn apply(&mut self, splice: &Splice) -> Splice {
        let inverse = splice.apply(&mut self.text);

        self.pins.apply(splice);
        self.modified = true;

        inverse
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
