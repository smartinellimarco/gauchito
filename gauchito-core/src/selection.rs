//! Selection — multi-cursor state, anchored by pins.
//!
//! A [`Range`] `{ anchor, head }` is two [`PinId`]s into the
//! buffer's [`PinTable`], so selections ride edits the same way
//! decorations do. "anchor" is the Helix-idiomatic selection-end
//! convention; the tracking store is `PinTable`, not "AnchorTable",
//! so one word doesn't carry two meanings (2026-05-26 review).
//!
//! A [`Selection`] is a non-empty list of ranges plus a primary
//! index. [`Selection::renormalize`] keeps the list sorted and
//! overlap-merged so multi-cursor invariants (no nested ranges,
//! deterministic order) hold after every mutation; the primary
//! index is repaired to whichever range contains the old primary's
//! span.
//!
//! Selection lives on [`crate::View`] rather than as parallel
//! decorations because the head/anchor asymmetry and the primary
//! index are awkward to express as a decoration row, and a typed
//! writer surface keeps `view:set_selection(...)` honest. The
//! renderer synthesizes transient `selection`-namespace decorations
//! from it at paint time — one storage shape, one render path.

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
