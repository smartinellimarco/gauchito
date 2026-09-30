//! Motion kernels — pure position-to-position functions.
//!
//! Each motion takes a `RopeSlice` and the current byte position and
//! returns a new byte position. Columns count chars. Kernels never mutate selection or buffer
//! state — that's the caller's job. This keeps them composable
//! (vim's `wb` is "word forward then word back") and trivially
//! testable.
//!
//! Vertical motion respects the `\n`-as-empty-line convention
//! ([`last_navigable_line`]): opening `"hello\nworld\n"` leaves a
//! navigable row past `world`. Without this, `paint.buffer` would
//! stop one row early and the cursor would have nowhere to land.
//!
//! [`CharClass`] (Eol / Whitespace / Word / Punct) drives word
//! motion; `_` counts as Word so identifiers stay one token under
//! `w` / `b`.

use ropey::{LineType, RopeSlice};

use crate::grapheme::{next_grapheme_boundary, prev_grapheme_boundary};

pub const LINES: LineType = LineType::LF_CR;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CharClass {
    Eol,
    Whitespace,
    Word,
    Punct,
}

pub fn char_class(c: char) -> CharClass {
    if c == '\n' || c == '\r' {
        CharClass::Eol
    } else if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punct
    }
}

pub fn move_grapheme(text: &RopeSlice, pos: usize, count: isize) -> usize {
    let mut p = pos;

    if count >= 0 {
        for _ in 0..count {
            p = next_grapheme_boundary(text, p);
        }
    } else {
        for _ in 0..count.unsigned_abs() {
            p = prev_grapheme_boundary(text, p);
        }
    }

    p
}

/// The index of the last line the user can navigate to.
///
/// A trailing `\n` creates a real, navigable empty line — vim-like.
/// Without this, opening a file `"hello\nworld\n"` would leave the
/// cursor unable to land below `"world"`, and `paint.buffer` would
/// stop one row early.
///
/// `ropey` counts an implicit trailing empty line after a final `\n`
/// (so `"hello\n"` has `len_lines == 2`). We expose that to distros.
pub fn last_navigable_line(text: &RopeSlice) -> usize {
    text.len_lines(LINES) - 1
}

pub fn line_of(text: &RopeSlice, pos: usize) -> usize {
    text.byte_to_line_idx(pos, LINES)
}

pub fn line_start(text: &RopeSlice, line: usize) -> usize {
    text.line_to_byte_idx(line, LINES)
}

fn visible_chars(text: &RopeSlice, line: usize) -> impl Iterator<Item = char> {
    text.line(line, LINES)
        .chars()
        .take_while(|&c| char_class(c) != CharClass::Eol)
}

// Byte position of the last visible char on `line`, or its start when empty.
fn last_visible(text: &RopeSlice, line: usize) -> usize {
    let start = line_start(text, line);
    let end = start + visible_chars(text, line).map(char::len_utf8).sum::<usize>();
    if end == start { start } else { text.floor_char_boundary(end - 1) }
}

pub fn move_vertical(
    text: &RopeSlice,
    pos: usize,
    count: isize,
    preferred_col: Option<usize>,
) -> usize {
    let line = line_of(text, pos).min(last_navigable_line(text));
    let col = preferred_col.unwrap_or_else(|| visual_column(text, pos));

    let new_line = if count >= 0 {
        line + count as usize
    } else {
        line.saturating_sub(count.unsigned_abs())
    };

    position_at(text, new_line, col)
}

pub fn skip_class_forward(text: &RopeSlice, pos: usize) -> usize {
    if pos >= text.len() {
        return pos;
    }

    let cls = char_class(text.char(pos));

    pos + text
        .chars_at(pos)
        .take_while(|&c| char_class(c) == cls)
        .map(char::len_utf8)
        .sum::<usize>()
}

pub fn skip_class_backward(text: &RopeSlice, pos: usize) -> usize {
    let mut chars = text.chars_at(pos);
    let Some(first) = chars.prev() else { return 0 };
    let cls = char_class(first);

    let mut p = pos - first.len_utf8();
    while let Some(c) = chars.prev() {
        if char_class(c) != cls {
            break;
        }
        p -= c.len_utf8();
    }
    p
}

pub fn move_left(text: &RopeSlice, head: usize) -> usize {
    move_grapheme(text, head, -1)
}

pub fn move_right(text: &RopeSlice, head: usize) -> usize {
    move_grapheme(text, head, 1)
}

pub fn move_left_inline(text: &RopeSlice, head: usize) -> usize {
    if head <= line_start(text, line_of(text, head)) {
        head
    } else {
        move_grapheme(text, head, -1)
    }
}

pub fn move_right_inline(text: &RopeSlice, head: usize) -> usize {
    if head >= last_visible(text, line_of(text, head)) {
        head
    } else {
        move_grapheme(text, head, 1)
    }
}

pub fn move_line_start(text: &RopeSlice, head: usize) -> usize {
    line_start(text, line_of(text, head))
}

pub fn move_line_end(text: &RopeSlice, head: usize) -> usize {
    last_visible(text, line_of(text, head))
}

pub fn move_first_non_whitespace(text: &RopeSlice, head: usize) -> usize {
    let line = line_of(text, head);

    line_start(text, line)
        + text
            .line(line, LINES)
            .chars()
            .take_while(|&c| char_class(c) == CharClass::Whitespace)
            .map(char::len_utf8)
            .sum::<usize>()
}

pub fn move_doc_end(text: &RopeSlice, _head: usize) -> usize {
    text.len()
}

pub fn move_to_line(text: &RopeSlice, line: usize) -> usize {
    line_start(text, line.min(last_navigable_line(text)))
}

pub fn visual_column(text: &RopeSlice, pos: usize) -> usize {
    text.slice(line_start(text, line_of(text, pos))..pos).chars().count()
}

pub fn position_at(text: &RopeSlice, line: usize, col: usize) -> usize {
    let line = line.min(last_navigable_line(text));
    let max_col = visible_line_chars(text, line).saturating_sub(1);

    line_start(text, line)
        + visible_chars(text, line)
            .take(col.min(max_col))
            .map(char::len_utf8)
            .sum::<usize>()
}

pub fn visible_line_chars(text: &RopeSlice, line: usize) -> usize {
    visible_chars(text, line).count()
}
