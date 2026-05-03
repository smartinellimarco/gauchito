//! Anchor-backed selections.
//!
//! `Range` holds two `AnchorId`s (anchor and head) into a document's
//! [`AnchorTable`]. Offsets are obtained by resolving the anchors against the
//! table. Edits to the rope automatically update every anchor in lockstep, so
//! selections track logical positions without per-selection mapping code.
//!
//! Lifecycle: any code that drops a `Range` (merge, collapse, remove) must
//! free its anchors via the table — these helpers all take `&mut AnchorTable`.

use crate::anchor::{AnchorId, AnchorTable};
use crate::changeset::Bias;
use crate::history::SelectionSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub anchor: AnchorId,
    pub head: AnchorId,
}

impl Range {
    /// Allocate a new range with anchor and head at the given offsets.
    pub fn new(t: &mut AnchorTable, anchor: usize, head: usize) -> Self {
        Self {
            anchor: t.create(anchor, Bias::After),
            head: t.create(head, Bias::After),
        }
    }

    /// Allocate a zero-width range at `pos`. Anchor and head are distinct
    /// anchors at the same offset so motions can move head without dragging
    /// anchor along.
    pub fn point(t: &mut AnchorTable, pos: usize) -> Self {
        Self::new(t, pos, pos)
    }

    /// Free both anchors. Call when discarding a range.
    pub fn drop(self, t: &mut AnchorTable) {
        t.drop(self.anchor);
        t.drop(self.head);
    }

    pub fn anchor_offset(&self, t: &AnchorTable) -> usize {
        t.offset(self.anchor)
    }

    pub fn head_offset(&self, t: &AnchorTable) -> usize {
        t.offset(self.head)
    }

    pub fn from(&self, t: &AnchorTable) -> usize {
        self.anchor_offset(t).min(self.head_offset(t))
    }

    pub fn to(&self, t: &AnchorTable) -> usize {
        self.anchor_offset(t).max(self.head_offset(t))
    }

    pub fn is_empty(&self, t: &AnchorTable) -> bool {
        self.anchor_offset(t) == self.head_offset(t)
    }

    pub fn len(&self, t: &AnchorTable) -> usize {
        self.to(t) - self.from(t)
    }

    pub fn is_forward(&self, t: &AnchorTable) -> bool {
        self.anchor_offset(t) <= self.head_offset(t)
    }

    pub fn flip(&mut self) {
        std::mem::swap(&mut self.anchor, &mut self.head);
    }

    pub fn contains(&self, t: &AnchorTable, pos: usize) -> bool {
        pos >= self.from(t) && pos < self.to(t)
    }

    pub fn overlaps(&self, t: &AnchorTable, other: &Self) -> bool {
        self.from(t) <= other.to(t) && other.from(t) <= self.to(t)
    }

    /// Move both anchor and head to specific offsets.
    pub fn set(&self, t: &mut AnchorTable, anchor: usize, head: usize) {
        t.set_offset(self.anchor, anchor);
        t.set_offset(self.head, head);
    }

    /// Move just the head; anchor stays put.
    pub fn set_head(&self, t: &mut AnchorTable, head: usize) {
        t.set_offset(self.head, head);
    }

    /// Move just the anchor; head stays put.
    pub fn set_anchor(&self, t: &mut AnchorTable, anchor: usize) {
        t.set_offset(self.anchor, anchor);
    }

    /// Absorb `other` into `self`, freeing `other`'s anchors and growing `self`
    /// to cover both ranges. Direction (forward/backward) is preserved from
    /// `self`.
    pub fn merge_with(self, t: &mut AnchorTable, other: Range) -> Range {
        let from = self.from(t).min(other.from(t));
        let to = self.to(t).max(other.to(t));
        let forward = self.is_forward(t);

        if forward {
            t.set_offset(self.anchor, from);
            t.set_offset(self.head, to);
        } else {
            t.set_offset(self.anchor, to);
            t.set_offset(self.head, from);
        }

        other.drop(t);
        self
    }
}

#[derive(Debug, Clone)]
pub struct Selection {
    ranges: Vec<Range>,
    primary: usize,
}

impl Selection {
    /// Build a selection from already-allocated ranges, normalizing as needed.
    pub fn new(t: &mut AnchorTable, ranges: Vec<Range>, primary: usize) -> Self {
        assert!(!ranges.is_empty(), "Selection must have at least one range");
        assert!(primary < ranges.len(), "primary index out of bounds");

        let mut sel = Self { ranges, primary };

        sel.normalize(t);

        sel
    }

    /// Allocate a fresh single-point selection at `pos`.
    pub fn point(t: &mut AnchorTable, pos: usize) -> Self {
        Self {
            ranges: vec![Range::point(t, pos)],
            primary: 0,
        }
    }

    /// Allocate a fresh single-range selection.
    pub fn single(t: &mut AnchorTable, anchor: usize, head: usize) -> Self {
        Self {
            ranges: vec![Range::new(t, anchor, head)],
            primary: 0,
        }
    }

    /// Free every anchor owned by this selection.
    pub fn drop(self, t: &mut AnchorTable) {
        for r in self.ranges {
            r.drop(t);
        }
    }

    /// Freeze the selection into resolved offsets. Used to store a selection
    /// in history records, where anchor ids cannot survive across rehydration.
    pub fn snapshot(&self, t: &AnchorTable) -> SelectionSnapshot {
        SelectionSnapshot {
            ranges: self
                .ranges
                .iter()
                .map(|r| (r.anchor_offset(t), r.head_offset(t)))
                .collect(),
            primary: self.primary,
        }
    }

    /// Allocate a fresh selection from a snapshot. Normalizes the result so
    /// overlapping / duplicate ranges from the snapshot are merged — the
    /// `Selection` invariant (sorted, disjoint ranges) is restored even if
    /// the snapshot didn't honour it.
    pub fn from_snapshot(t: &mut AnchorTable, snap: &SelectionSnapshot) -> Self {
        if snap.ranges.is_empty() {
            return Self::point(t, 0);
        }

        let ranges: Vec<Range> = snap
            .ranges
            .iter()
            .map(|&(a, h)| Range::new(t, a, h))
            .collect();

        let primary = snap.primary.min(ranges.len() - 1);

        Self::new(t, ranges, primary)
    }

    pub fn primary(&self) -> &Range {
        &self.ranges[self.primary]
    }

    pub fn ranges(&self) -> &[Range] {
        &self.ranges
    }

    pub fn ranges_mut(&mut self) -> &mut [Range] {
        &mut self.ranges
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn primary_idx(&self) -> usize {
        self.primary
    }

    pub fn set_primary(&mut self, idx: usize) {
        assert!(idx < self.ranges.len());

        self.primary = idx;
    }

    /// Add a range, keeping sorted order. Merges with neighbours on overlap.
    /// The new range becomes primary.
    pub fn push(&mut self, t: &mut AnchorTable, range: Range) {
        let from = range.from(t);
        let idx = self.ranges.partition_point(|r| r.from(t) < from);

        self.ranges.insert(idx, range);
        self.primary = idx;
        self.merge_around(t, idx);
    }

    /// Replace the primary range. Frees the previous primary's anchors.
    pub fn replace_primary(&mut self, t: &mut AnchorTable, range: Range) {
        let old = std::mem::replace(&mut self.ranges[self.primary], range);

        old.drop(t);

        let idx = self.primary;

        self.normalize_with_hint(t, idx);
    }

    /// Remove a range by index, freeing its anchors.
    pub fn remove(&mut self, t: &mut AnchorTable, idx: usize) {
        assert!(self.ranges.len() > 1, "cannot remove the last range");

        let removed = self.ranges.remove(idx);

        removed.drop(t);

        self.primary = self.primary.min(self.ranges.len() - 1);
    }

    /// Collapse all cursors to a single point at the primary head's offset.
    /// Frees every other range's anchors.
    pub fn collapse_to_primary(&mut self, t: &mut AnchorTable) {
        if self.ranges.len() == 1 {
            // Just collapse anchor onto head.
            let head_off = self.ranges[0].head_offset(t);

            self.ranges[0].set_anchor(t, head_off);

            return;
        }

        let head_off = self.primary().head_offset(t);
        let kept = self.ranges.swap_remove(self.primary);

        for r in self.ranges.drain(..) {
            r.drop(t);
        }

        kept.set(t, head_off, head_off);
        self.ranges = vec![kept];
        self.primary = 0;
    }

    // ── Normalization (private) ────────────────────────────────────────────

    /// Full sort + merge. Frees anchors for ranges absorbed by overlap.
    fn normalize(&mut self, t: &mut AnchorTable) {
        if self.ranges.len() <= 1 {
            return;
        }

        let primary_from = self.ranges[self.primary].from(t);
        let primary_to = self.ranges[self.primary].to(t);

        self.ranges.sort_unstable_by_key(|r| (r.from(t), r.to(t)));

        let mut merged: Vec<Range> = Vec::with_capacity(self.ranges.len());
        for range in self.ranges.drain(..) {
            match merged.last() {
                Some(last) if range.from(t) <= last.to(t) => {
                    let prev = merged.pop().unwrap();
                    merged.push(prev.merge_with(t, range));
                }
                _ => merged.push(range),
            }
        }

        let new_primary = merged
            .iter()
            .position(|r| r.from(t) <= primary_from && primary_to <= r.to(t))
            .unwrap_or(0);

        self.ranges = merged;
        self.primary = new_primary;
    }

    /// Bubble the range at `hint` into sorted position, then merge overlaps.
    fn normalize_with_hint(&mut self, t: &mut AnchorTable, hint: usize) {
        if self.ranges.len() <= 1 {
            return;
        }

        let mut idx = hint;

        while idx > 0 {
            let curr = (self.ranges[idx].from(t), self.ranges[idx].to(t));
            let prev = (self.ranges[idx - 1].from(t), self.ranges[idx - 1].to(t));

            if curr < prev {
                self.ranges.swap(idx, idx - 1);
                if self.primary == idx {
                    self.primary = idx - 1;
                } else if self.primary == idx - 1 {
                    self.primary = idx;
                }
                idx -= 1;
            } else {
                break;
            }
        }

        while idx + 1 < self.ranges.len() {
            let curr = (self.ranges[idx].from(t), self.ranges[idx].to(t));
            let next = (self.ranges[idx + 1].from(t), self.ranges[idx + 1].to(t));

            if curr > next {
                self.ranges.swap(idx, idx + 1);
                if self.primary == idx {
                    self.primary = idx + 1;
                } else if self.primary == idx + 1 {
                    self.primary = idx;
                }
                idx += 1;
            } else {
                break;
            }
        }

        self.merge_around(t, idx);
    }

    /// Merge the range at `idx` with neighbours on overlap, freeing absorbed ones.
    fn merge_around(&mut self, t: &mut AnchorTable, mut idx: usize) {
        while idx + 1 < self.ranges.len() && self.ranges[idx].overlaps(t, &self.ranges[idx + 1]) {
            let next = self.ranges.remove(idx + 1);
            let curr = self.ranges.remove(idx);

            self.ranges.insert(idx, curr.merge_with(t, next));

            if self.primary == idx + 1 {
                self.primary = idx;
            } else if self.primary > idx + 1 {
                self.primary -= 1;
            }
        }
        while idx > 0 && self.ranges[idx - 1].overlaps(t, &self.ranges[idx]) {
            let curr = self.ranges.remove(idx);
            let prev = self.ranges.remove(idx - 1);

            self.ranges.insert(idx - 1, prev.merge_with(t, curr));
            if self.primary == idx {
                self.primary = idx - 1;
            } else if self.primary > idx {
                self.primary -= 1;
            }

            idx -= 1;
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> AnchorTable {
        AnchorTable::new()
    }

    // ── Range helpers ─────────────────────────────────────────────────────────

    #[test]
    fn range_boundaries() {
        let mut t = at();
        let r = Range::new(&mut t, 5, 2);
        assert_eq!(r.from(&t), 2);
        assert_eq!(r.to(&t), 5);
        assert_eq!(r.len(&t), 3);
        assert!(!r.is_empty(&t));
        assert!(!r.is_forward(&t));
    }

    #[test]
    fn range_point_is_empty() {
        let mut t = at();
        let r = Range::point(&mut t, 7);
        assert!(r.is_empty(&t));
        assert_eq!(r.len(&t), 0);
        assert_eq!(r.from(&t), r.to(&t));
    }

    #[test]
    fn range_flip() {
        let mut t = at();
        let mut r = Range::new(&mut t, 3, 8);
        let from_before = r.from(&t);
        let to_before = r.to(&t);
        r.flip();
        assert_eq!(r.anchor_offset(&t), 8);
        assert_eq!(r.head_offset(&t), 3);
        assert_eq!(r.from(&t), from_before);
        assert_eq!(r.to(&t), to_before);
    }

    #[test]
    fn range_contains() {
        let mut t = at();
        let r = Range::new(&mut t, 2, 6);
        assert!(!r.contains(&t, 1));
        assert!(r.contains(&t, 2));
        assert!(r.contains(&t, 5));
        assert!(!r.contains(&t, 6));
    }

    #[test]
    fn range_overlaps() {
        let mut t = at();
        let a = Range::new(&mut t, 0, 5);
        let b = Range::new(&mut t, 4, 9);
        let c = Range::new(&mut t, 5, 9);
        let d = Range::new(&mut t, 6, 9);

        assert!(a.overlaps(&t, &b));
        assert!(a.overlaps(&t, &c));
        assert!(!a.overlaps(&t, &d));
    }

    #[test]
    fn range_merge_preserves_direction() {
        let mut t = at();
        let a = Range::new(&mut t, 0, 4);
        let b = Range::new(&mut t, 3, 7);
        let m = a.merge_with(&mut t, b);
        assert_eq!(m.anchor_offset(&t), 0);
        assert_eq!(m.head_offset(&t), 7);

        let mut t2 = at();
        let a_back = Range::new(&mut t2, 4, 0);
        let b2 = Range::new(&mut t2, 3, 7);
        let m2 = a_back.merge_with(&mut t2, b2);
        assert_eq!(m2.anchor_offset(&t2), 7);
        assert_eq!(m2.head_offset(&t2), 0);
    }

    #[test]
    fn merge_frees_other_anchors() {
        let mut t = at();
        let a = Range::new(&mut t, 0, 4);
        let b = Range::new(&mut t, 3, 7);
        let b_anchor = b.anchor;
        let b_head = b.head;
        let _m = a.merge_with(&mut t, b);
        assert!(t.try_offset(b_anchor).is_none());
        assert!(t.try_offset(b_head).is_none());
    }

    // ── Selection construction ────────────────────────────────────────────────

    #[test]
    fn selection_point() {
        let mut t = at();
        let s = Selection::point(&mut t, 10);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().head_offset(&t), 10);
    }

    #[test]
    fn selection_single() {
        let mut t = at();
        let s = Selection::single(&mut t, 2, 8);
        assert_eq!(s.primary().from(&t), 2);
        assert_eq!(s.primary().to(&t), 8);
    }

    #[test]
    fn selection_new_sorts_ranges() {
        let mut t = at();
        let ranges = vec![
            Range::new(&mut t, 10, 15),
            Range::new(&mut t, 0, 5),
            Range::new(&mut t, 6, 8),
        ];
        let s = Selection::new(&mut t, ranges, 0);
        let froms: Vec<usize> = s.ranges().iter().map(|r| r.from(&t)).collect();
        assert_eq!(froms, vec![0, 6, 10]);
    }

    #[test]
    fn selection_new_merges_overlapping() {
        let mut t = at();
        let ranges = vec![Range::new(&mut t, 0, 5), Range::new(&mut t, 3, 9)];
        let s = Selection::new(&mut t, ranges, 0);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().from(&t), 0);
        assert_eq!(s.primary().to(&t), 9);
    }

    #[test]
    fn selection_new_merges_touching() {
        let mut t = at();
        let ranges = vec![Range::new(&mut t, 0, 5), Range::new(&mut t, 5, 10)];
        let s = Selection::new(&mut t, ranges, 0);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn selection_primary_tracked_after_merge() {
        let mut t = at();
        let ranges = vec![Range::new(&mut t, 3, 9), Range::new(&mut t, 0, 5)];
        let s = Selection::new(&mut t, ranges, 0);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().from(&t), 0);
        assert_eq!(s.primary().to(&t), 9);
    }

    // ── push ──────────────────────────────────────────────────────────────────

    #[test]
    fn push_keeps_sorted_order() {
        let mut t = at();
        let mut s = Selection::single(&mut t, 10, 15);
        let r = Range::new(&mut t, 0, 5);
        s.push(&mut t, r);
        assert_eq!(s.ranges()[0].from(&t), 0);
        assert_eq!(s.ranges()[1].from(&t), 10);
    }

    #[test]
    fn push_new_range_becomes_primary() {
        let mut t = at();
        let mut s = Selection::single(&mut t, 10, 15);
        let r = Range::new(&mut t, 0, 5);
        s.push(&mut t, r);
        assert_eq!(s.primary().from(&t), 0);
    }

    #[test]
    fn push_merges_overlapping_new_range() {
        let mut t = at();
        let mut s = Selection::single(&mut t, 5, 10);
        let r = Range::new(&mut t, 8, 14);
        s.push(&mut t, r);
        assert_eq!(s.len(), 1);
        assert_eq!(s.ranges()[0].from(&t), 5);
        assert_eq!(s.ranges()[0].to(&t), 14);
    }

    #[test]
    fn push_multiple_non_overlapping() {
        let mut t = at();
        let mut s = Selection::point(&mut t, 0);
        let r5 = Range::point(&mut t, 5);
        s.push(&mut t, r5);
        let r10 = Range::point(&mut t, 10);
        s.push(&mut t, r10);
        let r3 = Range::point(&mut t, 3);
        s.push(&mut t, r3);
        assert_eq!(s.len(), 4);
        let froms: Vec<usize> = s.ranges().iter().map(|r| r.from(&t)).collect();
        assert_eq!(froms, vec![0, 3, 5, 10]);
    }

    #[test]
    fn push_point_inside_existing_merges() {
        let mut t = at();
        let mut s = Selection::single(&mut t, 0, 10);
        let p = Range::point(&mut t, 5);
        s.push(&mut t, p);
        assert_eq!(s.len(), 1);
    }

    // ── collapse ──────────────────────────────────────────────────────────────

    #[test]
    fn collapse_to_primary_removes_others() {
        let mut t = at();
        let ranges = vec![
            Range::new(&mut t, 0, 5),
            Range::new(&mut t, 10, 15),
            Range::new(&mut t, 20, 25),
        ];
        let mut s = Selection::new(&mut t, ranges, 1);
        s.collapse_to_primary(&mut t);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().head_offset(&t), 15);
        assert!(s.primary().is_empty(&t));
    }

    // ── remove ────────────────────────────────────────────────────────────────

    #[test]
    fn remove_range() {
        let mut t = at();
        let ranges = vec![
            Range::point(&mut t, 0),
            Range::point(&mut t, 5),
            Range::point(&mut t, 10),
        ];
        let mut s = Selection::new(&mut t, ranges, 1);
        s.remove(&mut t, 0);
        assert_eq!(s.len(), 2);
        assert_eq!(s.ranges()[0].from(&t), 5);
    }

    #[test]
    #[should_panic]
    fn remove_last_panics() {
        let mut t = at();
        let mut s = Selection::point(&mut t, 0);
        s.remove(&mut t, 0);
    }

    // ── edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn all_ranges_collapse_to_same_point() {
        let mut t = at();
        let ranges = vec![
            Range::point(&mut t, 5),
            Range::point(&mut t, 5),
            Range::point(&mut t, 5),
        ];
        let s = Selection::new(&mut t, ranges, 0);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().from(&t), 5);
    }

    #[test]
    fn backward_range_normalizes_correctly() {
        let mut t = at();
        let r = Range::new(&mut t, 10, 3);
        assert_eq!(r.from(&t), 3);
        assert_eq!(r.to(&t), 10);
        assert!(!r.is_forward(&t));
    }

    #[test]
    fn adjacent_non_overlapping_ranges_stay_separate() {
        let mut t = at();
        let ranges = vec![Range::new(&mut t, 0, 5), Range::new(&mut t, 6, 10)];
        let s = Selection::new(&mut t, ranges, 0);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn large_multicursor_stress() {
        let mut t = at();
        let mut s = Selection::point(&mut t, 0);
        for i in 1..100usize {
            let r = Range::point(&mut t, i * 10);
            s.push(&mut t, r);
        }
        assert_eq!(s.len(), 100);
        for (i, r) in s.ranges().iter().enumerate() {
            assert_eq!(r.from(&t), i * 10);
        }
    }

    #[test]
    fn cascading_merge_on_push() {
        let mut t = at();
        let ranges = vec![
            Range::new(&mut t, 0, 4),
            Range::new(&mut t, 6, 10),
            Range::new(&mut t, 12, 16),
        ];
        let mut s = Selection::new(&mut t, ranges, 0);
        let r = Range::new(&mut t, 3, 13);
        s.push(&mut t, r);
        assert_eq!(s.len(), 1);
        assert_eq!(s.ranges()[0].from(&t), 0);
        assert_eq!(s.ranges()[0].to(&t), 16);
    }

    #[test]
    #[should_panic]
    fn new_with_empty_vec_panics() {
        let mut t = at();
        Selection::new(&mut t, vec![], 0);
    }

    #[test]
    #[should_panic]
    fn new_with_bad_primary_panics() {
        let mut t = at();
        let r = Range::point(&mut t, 0);
        Selection::new(&mut t, vec![r], 5);
    }

    // ── Edits track via AnchorTable ──────────────────────────────────────────

    #[test]
    fn from_snapshot_merges_duplicate_collapsed_ranges() {
        // Two collapsed ranges at the same offset: from_snapshot must merge
        // them so the Selection invariant (disjoint ranges) holds.
        let mut t = at();
        let snap = SelectionSnapshot {
            ranges: vec![(5, 5), (5, 5), (5, 5)],
            primary: 0,
        };
        let s = Selection::from_snapshot(&mut t, &snap);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().head_offset(&t), 5);
    }

    #[test]
    fn from_snapshot_merges_overlapping_ranges() {
        let mut t = at();
        let snap = SelectionSnapshot {
            ranges: vec![(0, 10), (5, 15), (12, 20)],
            primary: 0,
        };
        let s = Selection::from_snapshot(&mut t, &snap);
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary().from(&t), 0);
        assert_eq!(s.primary().to(&t), 20);
    }

    #[test]
    fn anchors_advance_on_insert() {
        use crate::changeset::ChangeBuilder;
        use ropey::Rope;

        let mut t = at();
        let s = Selection::single(&mut t, 2, 6);

        let rope: Rope = "hello world".into();
        let mut b = ChangeBuilder::new(rope.len_chars());
        b.advance_to(0);
        b.insert("XX");
        let cs = b.finish();
        t.apply(&cs);

        // Range was 2..6, now 4..8.
        assert_eq!(s.primary().from(&t), 4);
        assert_eq!(s.primary().to(&t), 8);
    }
}
