use std::collections::HashMap;

pub use crate::ids::AnchorId;
use crate::mutation::{Mutation, phi};

#[derive(Debug, Clone)]
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

    pub fn apply(&mut self, mutations: &[Mutation]) {
        for m in mutations {
            for offset in self.entries.values_mut() {
                *offset = phi(atom, *offset);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_resolve() {
        let mut t = AnchorTable::new();
        let a = t.add(5);
        let b = t.add(10);

        assert_eq!(t.offset(a), 5);
        assert_eq!(t.offset(b), 10);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn set_offset_mutates_in_place() {
        let mut t = AnchorTable::new();
        let a = t.add(5);

        t.set_offset(a, 42);

        assert_eq!(t.offset(a), 42);
    }

    #[test]
    fn apply_atom_insert_shifts_after_point() {
        let mut t = AnchorTable::new();
        let before = t.add(2);
        let after = t.add(8);

        t.apply_atom(&Mutation::Insert {
            p: 5,
            text: "XX".into(),
        });

        assert_eq!(t.offset(before), 2);
        assert_eq!(t.offset(after), 10);
    }

    #[test]
    fn apply_atom_delete_clamps_and_shifts() {
        let mut t = AnchorTable::new();
        let outside = t.add(2);
        let inside = t.add(7);
        let after = t.add(10);

        t.apply_atom(&Mutation::Delete {
            p: 5,
            q: 8,
            deleted: "abc".into(),
        });

        assert_eq!(t.offset(outside), 2);
        assert_eq!(t.offset(inside), 5);
        assert_eq!(t.offset(after), 7);
    }

    #[test]
    fn inverse_atom_undoes_anchor_movement() {
        let mut t = AnchorTable::new();
        let id = t.add(3);
        let atom = Mutation::Insert {
            p: 1,
            text: "ZZ".into(),
        };

        t.apply_atom(&atom);
        assert_eq!(t.offset(id), 5);

        t.apply_atom(&atom.invert());
        assert_eq!(t.offset(id), 3);
    }
}
