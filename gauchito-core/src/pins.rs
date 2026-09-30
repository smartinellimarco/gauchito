use std::collections::HashMap;

use crate::splice::{Gravity, Splice, phi};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PinId(usize);

#[derive(Clone, Copy, Debug)]
struct Entry {
    offset: usize,
    gravity: Gravity,
}

#[derive(Clone, Default)]
pub struct PinTable {
    next_id: usize,
    entries: HashMap<PinId, Entry>,
    // Sorted by offset so apply only walks the pins at or after the splice.
    order: Vec<PinId>,
}

impl PinTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, offset: usize, gravity: Gravity) -> PinId {
        let id = PinId(self.next_id);
        self.next_id += 1;

        let entries = &self.entries;
        let pos = self.order.partition_point(|other| entries[other].offset <= offset);

        self.entries.insert(id, Entry { offset, gravity });
        self.order.insert(pos, id);
        id
    }

    pub fn add_with_gravity(&mut self, offset: usize, gravity: Gravity) -> PinId {
        self.add(offset, gravity)
    }

    pub fn remove(&mut self, id: PinId) {
        let Some(entry) = self.entries.get(&id).copied() else { return };
        if let Some(pos) = self.locate(id, entry) {
            self.order.remove(pos);
        }
        self.entries.remove(&id);
    }

    pub fn offset(&self, id: PinId) -> usize {
        self.entries[&id].offset
    }

    pub fn gravity(&self, id: PinId) -> Gravity {
        self.entries[&id].gravity
    }

    pub fn set_offset(&mut self, id: PinId, offset: usize) {
        let Some(old) = self.entries.get(&id).copied() else { return };
        if old.offset == offset {
            return;
        }
        self.reindex(id, old, Entry { offset, gravity: old.gravity });
    }

    pub fn set_gravity(&mut self, id: PinId, gravity: Gravity) {
        let Some(old) = self.entries.get(&id).copied() else { return };
        if old.gravity == gravity {
            return;
        }
        self.reindex(id, old, Entry { offset: old.offset, gravity });
    }

    pub fn apply(&mut self, s: &Splice) {
        let entries = &self.entries;
        let suffix_start = self.order.partition_point(|id| entries[id].offset < s.p());

        // Pins inside [p, q] collapse to p or p + n and can cross; pins past q keep their order.
        let mut left_collapse = Vec::new();
        let mut right_collapse = Vec::new();
        let mut tail = Vec::new();

        for i in suffix_start..self.order.len() {
            let id = self.order[i];
            let entry = self
                .entries
                .get_mut(&id)
                .expect("order mirrors entries");
            let was = entry.offset;
            entry.offset = phi(s, was, entry.gravity);

            if was <= s.q() {
                if entry.offset == s.p() {
                    left_collapse.push(id);
                } else {
                    right_collapse.push(id);
                }
            } else {
                tail.push(id);
            }
        }

        self.order.truncate(suffix_start);
        self.order.extend(left_collapse);
        self.order.extend(right_collapse);
        self.order.extend(tail);
    }

    pub fn pins_in(&self, from: usize, to: usize) -> impl Iterator<Item = PinId> + '_ {
        let entries = &self.entries;
        let start = self.order.partition_point(|id| entries[id].offset < from);
        let end = self.order.partition_point(|id| entries[id].offset < to);
        self.order[start..end].iter().copied()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn locate(&self, id: PinId, entry: Entry) -> Option<usize> {
        let entries = &self.entries;
        let start = self.order.partition_point(|other| entries[other].offset < entry.offset);
        for at in start..self.order.len() {
            let other = self.order[at];
            if other == id {
                return Some(at);
            }
            if entries[&other].offset != entry.offset {
                return None;
            }
        }
        None
    }

    fn reindex(&mut self, id: PinId, old: Entry, new: Entry) {
        if let Some(pos) = self.locate(id, old) {
            self.order.remove(pos);
        }
        let slot = self.entries.get_mut(&id).expect("entry was present a moment ago");
        *slot = new;

        let entries = &self.entries;
        let pos = self.order.partition_point(|other| entries[other].offset <= new.offset);
        self.order.insert(pos, id);
    }
}

#[cfg(test)]
mod tests {
    use crate::pins::PinTable;
    use crate::splice::{Gravity, Splice};

    #[test]
    fn add_and_resolve() {
        let mut t = PinTable::new();
        let a = t.add(5, Gravity::Right);
        let b = t.add(10, Gravity::Right);

        assert_eq!(t.offset(a), 5);
        assert_eq!(t.offset(b), 10);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn set_offset_mutates_in_place() {
        let mut t = PinTable::new();
        let a = t.add(5, Gravity::Right);

        t.set_offset(a, 42);

        assert_eq!(t.offset(a), 42);
    }

    #[test]
    fn apply_splice_insert_shifts_after_point() {
        let mut t = PinTable::new();
        let before = t.add(2, Gravity::Right);
        let after = t.add(8, Gravity::Right);

        t.apply(&Splice::new(5, 5, "XX".into()));

        assert_eq!(t.offset(before), 2);
        assert_eq!(t.offset(after), 10);
    }

    #[test]
    fn apply_splice_delete_clamps_and_shifts() {
        let mut t = PinTable::new();
        let outside = t.add(2, Gravity::Right);
        let inside = t.add(7, Gravity::Right);
        let after = t.add(10, Gravity::Right);

        t.apply(&Splice::new(5, 8, String::new()));

        assert_eq!(t.offset(outside), 2);
        assert_eq!(t.offset(inside), 5);
        assert_eq!(t.offset(after), 7);
    }

    #[test]
    fn inverse_splice_undoes_anchor_movement() {
        use crate::Buffer;
        use ropey::Rope;

        let mut buf = Buffer::new();
        buf.text = Rope::from_str("abcdef");

        let mut t = PinTable::new();
        let id = t.add(3, Gravity::Right);

        let splice = Splice::new(1, 1, "ZZ".into());
        let inverse = buf.apply(&splice);

        t.apply(&splice);
        assert_eq!(t.offset(id), 5);

        t.apply(&inverse);
        assert_eq!(t.offset(id), 3);
    }

    #[test]
    fn apply_leaves_pins_before_splice_untouched() {
        let mut t = PinTable::new();
        let p0 = t.add(0, Gravity::Right);
        let p3 = t.add(3, Gravity::Right);
        let p5 = t.add(5, Gravity::Right);

        t.apply(&Splice::new(10, 10, "ABCDE".into()));

        assert_eq!(t.offset(p0), 0);
        assert_eq!(t.offset(p3), 3);
        assert_eq!(t.offset(p5), 5);
    }

    #[test]
    fn pins_in_returns_offsets_in_range() {
        let mut t = PinTable::new();
        let a = t.add(0, Gravity::Right);
        let b = t.add(5, Gravity::Right);
        let c = t.add(10, Gravity::Right);
        let d = t.add(20, Gravity::Right);

        let got: Vec<_> = t.pins_in(5, 15).collect();
        assert_eq!(got, vec![b, c]);
        assert!(!got.contains(&a));
        assert!(!got.contains(&d));
    }

    #[test]
    fn pins_in_is_half_open() {
        let mut t = PinTable::new();
        let a = t.add(5, Gravity::Right);
        let _b = t.add(10, Gravity::Right);

        let got: Vec<_> = t.pins_in(5, 10).collect();
        assert_eq!(got, vec![a]);
    }

    #[test]
    fn mixed_gravity_inside_splice_rebuckets() {
        let mut t = PinTable::new();
        let right_at_3 = t.add(3, Gravity::Right);
        let left_at_5 = t.add(5, Gravity::Left);
        let right_at_7 = t.add(7, Gravity::Right);
        let left_at_9 = t.add(9, Gravity::Left);

        t.apply(&Splice::new(2, 10, "X".into()));

        assert_eq!(t.offset(left_at_5), 2);
        assert_eq!(t.offset(left_at_9), 2);
        assert_eq!(t.offset(right_at_3), 3);
        assert_eq!(t.offset(right_at_7), 3);

        use std::collections::HashSet;
        let lefts: HashSet<_> = t.pins_in(2, 3).collect();
        assert_eq!(lefts, HashSet::from([left_at_5, left_at_9]));

        let rights: HashSet<_> = t.pins_in(3, 4).collect();
        assert_eq!(rights, HashSet::from([right_at_3, right_at_7]));
    }

    #[test]
    fn remove_keeps_index_consistent() {
        let mut t = PinTable::new();
        let a = t.add(5, Gravity::Right);
        let b = t.add(10, Gravity::Right);
        let c = t.add(15, Gravity::Right);

        t.remove(b);

        let got: Vec<_> = t.pins_in(0, 100).collect();
        assert_eq!(got, vec![a, c]);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn set_offset_repositions_in_index() {
        let mut t = PinTable::new();
        let a = t.add(5, Gravity::Right);
        let b = t.add(10, Gravity::Right);
        let c = t.add(15, Gravity::Right);

        t.set_offset(b, 20);

        let got: Vec<_> = t.pins_in(0, 100).collect();
        assert_eq!(got, vec![a, c, b]);
    }

    #[test]
    fn apply_preserves_tail_order() {
        let mut t = PinTable::new();
        let p15 = t.add(15, Gravity::Right);
        let p20 = t.add(20, Gravity::Right);
        let p25 = t.add(25, Gravity::Right);

        t.apply(&Splice::new(0, 10, "AB".into()));

        assert_eq!(t.offset(p15), 7);
        assert_eq!(t.offset(p20), 12);
        assert_eq!(t.offset(p25), 17);

        let got: Vec<_> = t.pins_in(0, 100).collect();
        assert_eq!(got, vec![p15, p20, p25]);
    }

    #[test]
    fn author_pins_rebucket_by_origin() {
        let mut t = PinTable::new();
        let cursor = t.add(5, Gravity::Author);
        let left = t.add(5, Gravity::Left);
        let right = t.add(5, Gravity::Right);

        t.apply(&Splice::remote(5, 5, "XY".into()));
        assert_eq!((t.offset(cursor), t.offset(left), t.offset(right)), (5, 5, 7));

        t.apply(&Splice::new(5, 5, "Z".into()));
        assert_eq!((t.offset(cursor), t.offset(left), t.offset(right)), (6, 5, 8));

        t.remove(cursor);
        t.remove(left);
        t.remove(right);
        assert!(t.is_empty());
        assert_eq!(t.pins_in(0, 100).count(), 0);
    }
}
