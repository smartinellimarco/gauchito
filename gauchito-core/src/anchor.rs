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

        // Insert "XX" at position 5.
        t.apply_atom(&Mutation::new(5, 5, "XX".into()));

        assert_eq!(t.offset(before), 2);
        assert_eq!(t.offset(after), 10);
    }

    #[test]
    fn apply_atom_delete_clamps_and_shifts() {
        let mut t = AnchorTable::new();
        let outside = t.add(2);
        let inside = t.add(7);
        let after = t.add(10);

        // Delete [5, 8).
        t.apply_atom(&Mutation::new(5, 8, String::new()));

        assert_eq!(t.offset(outside), 2);
        assert_eq!(t.offset(inside), 5);
        assert_eq!(t.offset(after), 7);
    }

    #[test]
    fn inverse_atom_undoes_anchor_movement() {
        use ropey::Rope;

        let mut t = AnchorTable::new();
        let id = t.add(3);

        // Apply "ZZ" insert at 1, capturing the inverse from the rope.
        let mut rope = Rope::from_str("abcdef");
        let atom = Mutation::new(1, 1, "ZZ".into());
        let inverse = atom.apply(&mut rope);

        t.apply_atom(&atom);
        assert_eq!(t.offset(id), 5);

        t.apply_atom(&inverse);
        assert_eq!(t.offset(id), 3);
    }
}
