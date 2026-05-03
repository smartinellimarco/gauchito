//! Pure edit kernels: take text + cursor positions, return a [`ChangeSet`].
//!
//! Functions here don't mutate anything; the caller applies the returned
//! changeset (typically via [`crate::document::Document::apply`] or
//! [`gauchito_ui::EditorState::apply_edit`]). Multi-cursor support is built
//! in: each kernel takes either a `&[usize]` (heads) or `&[(usize, usize)]`
//! (ranges) and produces a single combined changeset.

use ropey::RopeSlice;

use crate::changeset::{ChangeBuilder, ChangeSet};

/// Insert a single character at every head without replacing the selection.
pub fn insert_char(text: &RopeSlice, heads: &[usize], ch: char) -> ChangeSet {
    let doc_len = text.len_chars();
    let insert_text = &ch.to_string();

    let mut positions: Vec<usize> = heads.to_vec();
    positions.sort();
    positions.dedup();

    let mut b = ChangeBuilder::new(doc_len);
    for &pos in &positions {
        b.advance_to(pos);
        b.insert(insert_text);
    }

    b.finish()
}

/// Delete one character backward at each cursor, or the range if non-empty.
/// `ranges` are `(from, to)` half-open intervals; collapsed ranges (`from == to`)
/// trigger a one-char backspace at `from`.
pub fn delete_char_backward(text: &RopeSlice, ranges: &[(usize, usize)]) -> ChangeSet {
    let doc_len = text.len_chars();

    let mut spans: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(from, to)| {
            if from == to {
                (from.saturating_sub(1), from)
            } else {
                (from, to)
            }
        })
        .collect();
    spans.sort();

    let mut b = ChangeBuilder::new(doc_len);
    for &(from, to) in &spans {
        if from == to {
            continue;
        }
        b.advance_to(from);
        b.delete(to - from);
    }

    b.finish()
}

/// Delete one character forward at each cursor, or the range if non-empty.
pub fn delete_char_forward(text: &RopeSlice, ranges: &[(usize, usize)]) -> ChangeSet {
    let doc_len = text.len_chars();

    let mut spans: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(from, to)| {
            if from == to {
                (from, (from + 1).min(doc_len))
            } else {
                (from, to)
            }
        })
        .collect();
    spans.sort();

    let mut b = ChangeBuilder::new(doc_len);
    for &(from, to) in &spans {
        if from == to {
            continue;
        }
        b.advance_to(from);
        b.delete(to - from);
    }

    b.finish()
}

/// Delete the selected ranges. Collapsed ranges are ignored.
/// Returns the identity changeset if every range is collapsed.
pub fn delete_selection(text: &RopeSlice, ranges: &[(usize, usize)]) -> ChangeSet {
    let doc_len = text.len_chars();

    let mut spans: Vec<(usize, usize)> = ranges
        .iter()
        .copied()
        .filter(|&(from, to)| from != to)
        .collect();

    if spans.is_empty() {
        return ChangeSet::identity(doc_len);
    }
    spans.sort();

    let mut b = ChangeBuilder::new(doc_len);
    for &(from, to) in &spans {
        b.advance_to(from);
        b.delete(to - from);
    }

    b.finish()
}
