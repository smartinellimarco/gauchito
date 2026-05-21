use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::atom::{Atom, phi};

static NEXT_ANCHOR_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorId(pub usize);

impl AnchorId {
    pub fn next() -> Self {
        AnchorId(NEXT_ANCHOR_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Default)]
pub struct AnchorTable {
    entries: HashMap<AnchorId, usize>,
}

impl AnchorTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, offset: usize) -> AnchorId {
        let id = AnchorId::next();
        self.entries.insert(id, offset);
        id
    }

    pub fn remove(&mut self, id: AnchorId) {
        self.entries.remove(&id);
    }

    pub fn offset(&self, id: AnchorId) -> usize {
        self.entries[&id]
    }

    pub fn set_offset(&mut self, id: AnchorId, offset: usize) {
        if let Some(entry) = self.entries.get_mut(&id) {
            *entry = offset;
        }
    }

    pub fn apply(&mut self, m: &Atom) {
        for offset in self.entries.values_mut() {
            *offset = phi(m, *offset);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
