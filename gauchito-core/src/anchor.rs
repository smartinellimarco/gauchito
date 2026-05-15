use std::collections::HashMap;

pub use crate::ids::AnchorId;
use crate::mutation::{Mutation, phi};

#[derive(Clone)]
pub struct AnchorTable {
    entries: HashMap<AnchorId, usize>,
}

impl AnchorTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
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

    pub fn apply_atom(&mut self, m: &Mutation) {
        for offset in self.entries.values_mut() {
            *offset = phi(m, *offset);
        }
    }

    pub fn apply(&mut self, atoms: &[Mutation]) {
        for m in atoms {
            self.apply_atom(m);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AnchorTable {
    fn default() -> Self {
        Self::new()
    }
}
