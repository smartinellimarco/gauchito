// Operational Transformation for concurrent editing.
//
// Based on the Jupiter collaboration system:
// Nichols et al., "High-Latency, Low-Bandwidth Windowing in the Jupiter
// Collaboration System", 1995.
// https://doi.org/10.1145/215585.215706
//
// TP1: apply(a) · apply(xf(b, a)) ≡ apply(b) · apply(xf(a, b))

use super::Edit;

/// Transform edit `a` against already-applied edit `b`.
/// Returns `a'` — the adjusted edit achieving `a`'s intent on the post-`b` doc.
pub fn transform(a: &Edit, b: &Edit) -> Edit {
    let ins = b.text.len();
    let delta = b.delta();

    // a entirely before b
    if a.end < b.start {
        return a.clone();
    }

    // same-position inserts: lexicographic tie-break for TP1
    if a.start == b.start && a.is_insert() && b.is_insert() {
        return if a.text <= b.text {
            a.clone()
        } else {
            Edit::new(a.start + ins, a.end + ins, a.text.clone())
        };
    }

    // a adjacent after b's start (no overlap)
    if a.end == b.start {
        return a.clone();
    }

    // a entirely after b
    if a.start >= b.end {
        return Edit::new(
            (a.start as isize + delta).max(0) as usize,
            (a.end as isize + delta).max(0) as usize,
            a.text.clone(),
        );
    }

    // — overlapping cases —

    if a.start < b.start {
        if a.end <= b.end {
            // a: [-----)
            // b:    [-----)
            Edit::new(a.start, b.start, a.text.clone())
        } else {
            // a: [----------)
            // b:    [----)
            let tail = a.end - b.end;
            Edit::new(a.start, b.start + ins + tail, a.text.clone())
        }
    } else if a.end <= b.end {
        // a:   [---)
        // b: [-------)
        Edit::new(b.start + ins, b.start + ins, a.text.clone())
    } else {
        // a:    [------)
        // b: [-----)
        let tail = a.end - b.end;
        Edit::new(b.start + ins, b.start + ins + tail, a.text.clone())
    }
}

/// Transform each edit in `a` against all edits in `b`.
pub fn transform_list(a: &[Edit], b: &[Edit]) -> Vec<Edit> {
    a.iter()
        .map(|ea| {
            let mut t = ea.clone();
            for eb in b {
                t = transform(&t, eb);
            }
            t
        })
        .collect()
}

/// Transform a cursor/anchor position against an already-applied edit.
pub fn transform_pos(pos: usize, edit: &Edit) -> usize {
    if pos <= edit.start {
        pos
    } else if pos >= edit.end {
        (pos as isize + edit.delta()).max(0) as usize
    } else {
        edit.start + edit.text.len()
    }
}

#[cfg(test)]
#[path = "ot_tests.rs"]
mod tests;
