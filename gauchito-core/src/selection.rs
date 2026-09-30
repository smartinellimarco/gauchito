use crate::pins::{PinId, PinTable};
use crate::splice::Gravity;

#[derive(Clone)]
pub struct SelectionSnapshot {
    pub ranges: Vec<(usize, usize)>,
    pub primary: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub anchor: PinId,
    pub head: PinId,
}

impl Range {
    pub fn new(t: &mut PinTable, anchor: usize, head: usize) -> Self {
        Self {
            anchor: t.add(anchor, Gravity::Author),
            head: t.add(head, Gravity::Author),
        }
    }

    pub fn point(t: &mut PinTable, pos: usize) -> Self {
        Self::new(t, pos, pos)
    }

    pub fn drop(self, t: &mut PinTable) {
        t.remove(self.anchor);
        t.remove(self.head);
    }

    pub fn anchor_offset(&self, t: &PinTable) -> usize {
        t.offset(self.anchor)
    }

    pub fn head_offset(&self, t: &PinTable) -> usize {
        t.offset(self.head)
    }

    pub fn offsets(&self, t: &PinTable) -> (usize, usize) {
        (self.anchor_offset(t), self.head_offset(t))
    }

    pub fn from(&self, t: &PinTable) -> usize {
        self.anchor_offset(t).min(self.head_offset(t))
    }

    pub fn to(&self, t: &PinTable) -> usize {
        self.anchor_offset(t).max(self.head_offset(t))
    }

    pub fn is_empty(&self, t: &PinTable) -> bool {
        self.anchor_offset(t) == self.head_offset(t)
    }

    pub fn len(&self, t: &PinTable) -> usize {
        self.to(t) - self.from(t)
    }

    pub fn is_forward(&self, t: &PinTable) -> bool {
        self.anchor_offset(t) <= self.head_offset(t)
    }

    pub fn flip(&mut self) {
        std::mem::swap(&mut self.anchor, &mut self.head);
    }

    pub fn contains(&self, t: &PinTable, pos: usize) -> bool {
        pos >= self.from(t) && pos < self.to(t)
    }

    pub fn overlaps(&self, t: &PinTable, other: &Self) -> bool {
        self.from(t) <= other.to(t) && other.from(t) <= self.to(t)
    }

    pub fn set(&self, t: &mut PinTable, anchor: usize, head: usize) {
        t.set_offset(self.anchor, anchor);
        t.set_offset(self.head, head);
    }

    pub fn set_head(&self, t: &mut PinTable, head: usize) {
        t.set_offset(self.head, head);
    }

    pub fn set_anchor(&self, t: &mut PinTable, anchor: usize) {
        t.set_offset(self.anchor, anchor);
    }
}

#[derive(Debug, Clone)]
pub struct Selection {
    ranges: Vec<Range>,
    primary: usize,
}

impl Selection {
    pub fn new(t: &mut PinTable, ranges: Vec<Range>, primary: usize) -> Self {
        assert!(!ranges.is_empty(), "Selection must have at least one range");
        assert!(primary < ranges.len(), "primary index out of bounds");

        let mut sel = Self { ranges, primary };
        sel.renormalize(t);
        sel
    }

    pub fn point(t: &mut PinTable, pos: usize) -> Self {
        Self {
            ranges: vec![Range::point(t, pos)],
            primary: 0,
        }
    }

    pub fn single(t: &mut PinTable, anchor: usize, head: usize) -> Self {
        Self {
            ranges: vec![Range::new(t, anchor, head)],
            primary: 0,
        }
    }

    pub fn drop(&mut self, t: &mut PinTable) {
        for r in std::mem::take(&mut self.ranges) {
            r.drop(t);
        }
    }

    pub fn snapshot(&self, t: &PinTable) -> SelectionSnapshot {
        SelectionSnapshot {
            ranges: self.ranges.iter().map(|r| r.offsets(t)).collect(),
            primary: self.primary,
        }
    }

    pub fn from_snapshot(t: &mut PinTable, snap: &SelectionSnapshot) -> Self {
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

    pub fn push(&mut self, t: &mut PinTable, range: Range) {
        self.ranges.push(range);
        self.primary = self.ranges.len() - 1;
        self.renormalize(t);
    }

    pub fn replace_primary(&mut self, t: &mut PinTable, range: Range) {
        let old = std::mem::replace(&mut self.ranges[self.primary], range);
        old.drop(t);
        self.renormalize(t);
    }

    pub fn remove(&mut self, t: &mut PinTable, idx: usize) {
        assert!(self.ranges.len() > 1, "cannot remove the last range");

        let removed = self.ranges.remove(idx);
        removed.drop(t);

        if self.primary > idx || self.primary >= self.ranges.len() {
            self.primary = self.primary.saturating_sub(1).min(self.ranges.len() - 1);
        }
    }

    pub fn collapse_to_primary(&mut self, t: &mut PinTable) {
        if self.ranges.len() == 1 {
            let head = self.ranges[0].head_offset(t);
            self.ranges[0].set(t, head, head);
            return;
        }

        let head = self.primary().head_offset(t);
        let kept = self.ranges.swap_remove(self.primary);
        for r in self.ranges.drain(..) {
            r.drop(t);
        }
        kept.set(t, head, head);
        self.ranges = vec![kept];
        self.primary = 0;
    }

    fn renormalize(&mut self, t: &mut PinTable) {
        if self.ranges.len() <= 1 {
            return;
        }

        let (pa, ph) = self.ranges[self.primary].offsets(t);
        let plo = pa.min(ph);
        let phi = pa.max(ph);

        let offsets: Vec<(usize, usize)> = self.ranges.iter().map(|r| r.offsets(t)).collect();
        let normalized = normalize_offsets(offsets);

        let new_primary = normalized
            .iter()
            .position(|&(a, h)| {
                let lo = a.min(h);
                let hi = a.max(h);
                lo <= plo && phi <= hi
            })
            .unwrap_or(0);

        for r in self.ranges.drain(..) {
            r.drop(t);
        }

        self.ranges = normalized
            .into_iter()
            .map(|(a, h)| Range::new(t, a, h))
            .collect();
        self.primary = new_primary;
    }
}

fn normalize_offsets(mut sels: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if sels.len() <= 1 {
        return sels;
    }

    sels.sort_by_key(|&(a, h)| (a.min(h), a.max(h)));

    let mut out: Vec<(usize, usize)> = Vec::with_capacity(sels.len());
    for (a, h) in sels {
        let (lo, hi) = (a.min(h), a.max(h));
        if let Some(&(la, lh)) = out.last() {
            let (llo, lhi) = (la.min(lh), la.max(lh));

            if lo <= lhi {
                let new_lo = llo.min(lo);
                let new_hi = lhi.max(hi);
                let forward = la <= lh;

                *out.last_mut().unwrap() = if forward {
                    (new_lo, new_hi)
                } else {
                    (new_hi, new_lo)
                };

                continue;
            }
        }
        out.push((a, h));
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::pins::PinTable;
    use crate::selection::SelectionSnapshot;
    use crate::selection::{Range, Selection};

    fn at() -> PinTable {
        PinTable::new()
    }

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

    #[test]
    fn from_snapshot_merges_duplicate_collapsed_ranges() {
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
        use crate::splice::Splice;

        let mut t = at();
        let s = Selection::single(&mut t, 2, 6);

        t.apply(&Splice::new(0, 0, "XX".into()));

        assert_eq!(s.primary().from(&t), 4);
        assert_eq!(s.primary().to(&t), 8);
    }
}
