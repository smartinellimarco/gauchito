use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DOCUMENT_ID: AtomicUsize = AtomicUsize::new(1);
static NEXT_VIEW_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub usize);

impl DocumentId {
    pub fn next() -> Self {
        DocumentId(NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl ViewId {
    pub fn next() -> Self {
        ViewId(NEXT_VIEW_ID.fetch_add(1, Ordering::Relaxed))
    }
}
