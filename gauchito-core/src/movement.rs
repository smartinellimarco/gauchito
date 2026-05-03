//! Pure motion algorithms over a [`RopeSlice`].
//!
//! Every function here is a plain offset-level computation: takes a slice
//! plus a position (and sometimes an anchor / char), returns a new position
//! (or `(anchor, head)` pair). No editor state, no history, no selections —
//! just text.
//!
//! Function shapes:
//! - Position:        `fn(&RopeSlice, head)            -> head`
//! - Selection-shape: `fn(&RopeSlice, anchor, head)    -> (anchor, head)`
//! - Char search:     `fn(&RopeSlice, head, ch)        -> head`
//!
//! Functions that "search" (find_char_*, match_bracket) return the original
//! position when the target is missing — so they're never partial.

use ropey::RopeSlice;

use crate::grapheme::{next_grapheme_boundary, prev_grapheme_boundary};

// ── Character classification ─────────────────────────────────────────────────

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

/// True if `c` is a WORD character (anything that isn't whitespace or line ending).
fn is_big_word(c: char) -> bool {
    !matches!(char_class(c), CharClass::Whitespace | CharClass::Eol)
}

// ── Bracket pairs ───────────────────────────────────────────────────────────

pub const BRACKET_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

/// Returns `(matching_char, is_forward)`, or `None` if not a bracket.
fn bracket_pair(c: char) -> Option<(char, bool)> {
    for &(open, close) in BRACKET_PAIRS {
        if c == open {
            return Some((close, true));
        }
        if c == close {
            return Some((open, false));
        }
    }
    None
}

// ── Grapheme primitives ─────────────────────────────────────────────────────

/// Step `count` graphemes. Positive = forward, negative = backward.
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

/// Last navigable line. A file ending with `\n` has an "empty trailing line"
/// in ropey's `len_lines()` model — vim convention treats that final `\n` as
/// a line terminator, not a line creator, so the empty trailing line is not
/// reachable via cursor motion.
pub fn last_navigable_line(text: &RopeSlice) -> usize {
    let len = text.len_chars();
    if len == 0 {
        return 0;
    }
    let lines = text.len_lines();
    if lines <= 1 {
        return 0;
    }
    let last = text.char(len - 1);
    if last == '\n' || last == '\r' {
        lines - 2
    } else {
        lines - 1
    }
}

/// Step `count` lines, preserving column. Positive = down, negative = up.
pub fn move_vertical(
    text: &RopeSlice,
    pos: usize,
    count: isize,
    preferred_col: Option<usize>,
) -> usize {
    let last_line = last_navigable_line(text);
    // Clamp the starting line in case `pos` is on the phantom trailing line.
    let line = text.char_to_line(pos).min(last_line);

    let col = preferred_col.unwrap_or_else(|| pos.saturating_sub(text.line_to_char(line)));

    let new_line = if count >= 0 {
        (line + count as usize).min(last_line)
    } else {
        line.saturating_sub(count.unsigned_abs())
    };

    let visible = visible_line_chars(text, new_line);
    let new_col = col.min(visible.saturating_sub(1));

    text.line_to_char(new_line) + new_col
}

/// Skip forward over a contiguous run of the same char class.
pub fn skip_class_forward(text: &RopeSlice, pos: usize) -> usize {
    let len = text.len_chars();
    if pos >= len {
        return pos;
    }

    let cls = char_class(text.char(pos));

    let mut p = pos;
    while p < len && char_class(text.char(p)) == cls {
        p += 1;
    }
    p
}

/// Skip backward over a contiguous run of the same char class.
pub fn skip_class_backward(text: &RopeSlice, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }

    let cls = char_class(text.char(pos - 1));

    let mut p = pos;
    while p > 0 && char_class(text.char(p - 1)) == cls {
        p -= 1;
    }
    p
}

/// Skip whitespace, cross at most one line ending, skip whitespace again.
fn skip_whitespace_and_newline(text: &RopeSlice, pos: usize) -> usize {
    let len = text.len_chars();
    if pos >= len {
        return pos;
    }

    let mut p = pos;

    if char_class(text.char(p)) == CharClass::Whitespace {
        p = skip_class_forward(text, p);
    }

    if p < len && char_class(text.char(p)) == CharClass::Eol {
        p = move_grapheme(text, p, 1);

        if p < len && char_class(text.char(p)) == CharClass::Whitespace {
            p = skip_class_forward(text, p);
        }
    }

    p
}

/// Skip whitespace and at most one line ending backward, skip whitespace again.
fn skip_whitespace_and_newline_backward(text: &RopeSlice, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }

    let mut p = pos;

    if char_class(text.char(p - 1)) == CharClass::Whitespace {
        p = skip_class_backward(text, p);
    }

    if p > 0 && char_class(text.char(p - 1)) == CharClass::Eol {
        p = move_grapheme(text, p, -1);

        if p > 0 && char_class(text.char(p - 1)) == CharClass::Whitespace {
            p = skip_class_backward(text, p);
        }
    }

    p
}

// ── Cursor movement (h/l/j/k) ────────────────────────────────────────────────

pub fn move_left(text: &RopeSlice, head: usize) -> usize {
    move_grapheme(text, head, -1)
}

pub fn move_right(text: &RopeSlice, head: usize) -> usize {
    move_grapheme(text, head, 1)
}

/// Move left one grapheme, but stay within the current line.
pub fn move_left_inline(text: &RopeSlice, head: usize) -> usize {
    let line = text.char_to_line(head);
    let line_start = text.line_to_char(line);
    if head <= line_start {
        head
    } else {
        move_grapheme(text, head, -1)
    }
}

/// Move right one grapheme, but stop at the last visible char on the line.
pub fn move_right_inline(text: &RopeSlice, head: usize) -> usize {
    let line = text.char_to_line(head);
    let visible = visible_line_chars(text, line);
    let line_start = text.line_to_char(line);
    let last_visible = if visible == 0 {
        line_start
    } else {
        line_start + visible - 1
    };
    if head >= last_visible {
        head
    } else {
        move_grapheme(text, head, 1)
    }
}

pub fn move_up(text: &RopeSlice, head: usize) -> usize {
    move_vertical(text, head, -1, None)
}

pub fn move_down(text: &RopeSlice, head: usize) -> usize {
    move_vertical(text, head, 1, None)
}

// ── Word motions ─────────────────────────────────────────────────────────────

/// `w` — move to the start of the next word.
pub fn move_word_forward(text: &RopeSlice, head: usize) -> usize {
    let len = text.len_chars();
    let mut p = head;

    if p < len {
        let cls = char_class(text.char(p));

        if matches!(cls, CharClass::Whitespace | CharClass::Eol) {
            p = skip_whitespace_and_newline(text, p);
        } else {
            p = skip_class_forward(text, p);
            p = skip_whitespace_and_newline(text, p);
        }
    }

    p.min(len)
}

/// `b` — move to the start of the previous word.
pub fn move_word_backward(text: &RopeSlice, head: usize) -> usize {
    let p = skip_whitespace_and_newline_backward(text, head);
    skip_class_backward(text, p)
}

/// `e` — move to the end of the current/next word.
pub fn move_word_end(text: &RopeSlice, head: usize) -> usize {
    let len = text.len_chars();
    let mut p = head;

    if p + 1 < len {
        p = move_grapheme(text, p, 1);
    }

    p = skip_whitespace_and_newline(text, p);

    if p < len {
        p = skip_class_forward(text, p);
    }

    p.saturating_sub(1).min(len)
}

/// `W` — move to the start of the next WORD (non-whitespace run).
pub fn move_word_forward_big(text: &RopeSlice, head: usize) -> usize {
    let len = text.len_chars();
    let mut p = head;

    while p < len && is_big_word(text.char(p)) {
        p += 1;
    }

    p = skip_whitespace_and_newline(text, p);
    p.min(len)
}

/// `B` — move to the start of the previous WORD.
pub fn move_word_backward_big(text: &RopeSlice, head: usize) -> usize {
    let mut p = skip_whitespace_and_newline_backward(text, head);

    while p > 0 && is_big_word(text.char(p - 1)) {
        p -= 1;
    }

    p
}

/// `E` — move to the end of the next WORD.
pub fn move_word_end_big(text: &RopeSlice, head: usize) -> usize {
    let len = text.len_chars();
    let mut p = head;

    if p + 1 < len {
        p = move_grapheme(text, p, 1);
    }

    p = skip_whitespace_and_newline(text, p);

    while p < len && is_big_word(text.char(p)) {
        p += 1;
    }

    p.saturating_sub(1).min(len)
}

// ── Line / document motions ──────────────────────────────────────────────────

/// `0` — move to the start of the current line.
pub fn move_line_start(text: &RopeSlice, head: usize) -> usize {
    text.line_to_char(text.char_to_line(head))
}

/// `$` — move to the last visible character on the current line.
pub fn move_line_end(text: &RopeSlice, head: usize) -> usize {
    let line = text.char_to_line(head);
    let line_start = text.line_to_char(line);
    let visible = visible_line_chars(text, line);

    if visible == 0 {
        line_start
    } else {
        line_start + visible - 1
    }
}

/// `^` — move to the first non-whitespace character on the current line.
pub fn move_first_non_whitespace(text: &RopeSlice, head: usize) -> usize {
    let line = text.char_to_line(head);
    let line_start = text.line_to_char(line);

    let offset = text
        .line(line)
        .chars()
        .take_while(|&c| char_class(c) == CharClass::Whitespace)
        .count();

    line_start + offset
}

/// `gg` — jump to document start.
pub fn move_doc_start(_text: &RopeSlice, _head: usize) -> usize {
    0
}

/// `G` — jump to document end.
pub fn move_doc_end(text: &RopeSlice, _head: usize) -> usize {
    text.len_chars()
}

/// First character of line `line` (0-indexed), clamped to document bounds.
pub fn move_to_line(text: &RopeSlice, line: usize) -> usize {
    let last_line = text.len_lines().saturating_sub(1);
    text.line_to_char(line.min(last_line))
}

/// Count visible (non-Eol) characters on a given line.
pub fn visible_line_chars(text: &RopeSlice, line: usize) -> usize {
    text.line(line)
        .chars()
        .take_while(|&c| char_class(c) != CharClass::Eol)
        .count()
}

// ── Paragraph motions ───────────────────────────────────────────────────────

/// A line is blank if it starts with Eol or is past the end of the document.
fn is_blank_line(text: &RopeSlice, line: usize) -> bool {
    let start = text.line_to_char(line);
    start >= text.len_chars() || char_class(text.char(start)) == CharClass::Eol
}

/// `}` — move to the next blank line.
pub fn move_paragraph_forward(text: &RopeSlice, head: usize) -> usize {
    let last_line = text.len_lines().saturating_sub(1);
    let mut line = text.char_to_line(head);

    while line < last_line && is_blank_line(text, line) {
        line += 1;
    }
    while line < last_line && !is_blank_line(text, line) {
        line += 1;
    }

    text.line_to_char(line)
}

/// `{` — move to the previous blank line.
pub fn move_paragraph_backward(text: &RopeSlice, head: usize) -> usize {
    let mut line = text.char_to_line(head);

    while line > 0 && is_blank_line(text, line) {
        line -= 1;
    }
    while line > 0 && !is_blank_line(text, line) {
        line -= 1;
    }

    text.line_to_char(line)
}

// ── Character search ────────────────────────────────────────────────────────

/// Internal: find the next `ch` after `pos` on the same line. Returns the
/// position, or `None` if missing / Eol crossed.
fn find_char_forward_pos(text: &RopeSlice, pos: usize, ch: char) -> Option<usize> {
    let len = text.len_chars();
    let mut p = pos + 1;

    while p < len {
        let c = text.char(p);
        if char_class(c) == CharClass::Eol {
            return None;
        }
        if c == ch {
            return Some(p);
        }
        p += 1;
    }

    None
}

/// Internal: find the previous `ch` before `pos` on the same line.
fn find_char_backward_pos(text: &RopeSlice, pos: usize, ch: char) -> Option<usize> {
    let mut p = pos;

    while p > 0 {
        p -= 1;
        let c = text.char(p);
        if char_class(c) == CharClass::Eol {
            return None;
        }
        if c == ch {
            return Some(p);
        }
    }

    None
}

/// `f` — move to next occurrence of `ch` on current line; stay on miss.
pub fn find_char_forward(text: &RopeSlice, head: usize, ch: char) -> usize {
    find_char_forward_pos(text, head, ch).unwrap_or(head)
}

/// `F` — move to previous occurrence of `ch` on current line; stay on miss.
pub fn find_char_backward(text: &RopeSlice, head: usize, ch: char) -> usize {
    find_char_backward_pos(text, head, ch).unwrap_or(head)
}

/// `t` — move to one before the next occurrence of `ch`; stay on miss.
pub fn find_char_forward_before(text: &RopeSlice, head: usize, ch: char) -> usize {
    match find_char_forward_pos(text, head, ch) {
        Some(p) => move_grapheme(text, p, -1).max(head),
        None => head,
    }
}

/// `T` — move to one after the previous occurrence of `ch`; stay on miss.
pub fn find_char_backward_after(text: &RopeSlice, head: usize, ch: char) -> usize {
    match find_char_backward_pos(text, head, ch) {
        Some(p) => move_grapheme(text, p, 1).min(head),
        None => head,
    }
}

// ── Bracket matching ────────────────────────────────────────────────────────

/// Internal: locate the matching bracket if `pos` is on one.
fn matching_bracket_pos(text: &RopeSlice, pos: usize) -> Option<usize> {
    let len = text.len_chars();
    if pos >= len {
        return None;
    }

    let c = text.char(pos);
    let (target, forward) = bracket_pair(c)?;

    let mut depth: usize = 1;

    if forward {
        let mut p = pos + 1;
        while p < len {
            let ch = text.char(p);
            if ch == c {
                depth += 1;
            } else if ch == target {
                depth -= 1;
                if depth == 0 {
                    return Some(p);
                }
            }
            p += 1;
        }
    } else {
        let mut p = pos;
        while p > 0 {
            p -= 1;
            let ch = text.char(p);
            if ch == c {
                depth += 1;
            } else if ch == target {
                depth -= 1;
                if depth == 0 {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// `%` — jump to matching bracket; stay on miss / non-bracket.
pub fn match_bracket(text: &RopeSlice, head: usize) -> usize {
    matching_bracket_pos(text, head).unwrap_or(head)
}

// ── Selection-shape primitives ──────────────────────────────────────────────

/// If head is on a line-ending character, snap it to the last visible char.
/// On an empty line, stays on the `\n` (only position available).
pub fn clamp_visible(text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let len = text.len_chars();
    if len == 0 {
        return (anchor, head);
    }

    let h = head.min(len - 1);
    let ch = text.char(h);
    if ch == '\n' || ch == '\r' {
        let line = text.char_to_line(h);
        let line_start = text.line_to_char(line);
        let visible = visible_line_chars(text, line);
        let clamped = if visible == 0 {
            line_start
        } else {
            line_start + visible - 1
        };
        (anchor, clamped)
    } else {
        (anchor, h)
    }
}

/// If the selection is collapsed, extend head forward by one so it covers
/// the character under the cursor.
pub fn ensure_char_selected(text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let len = text.len_chars();
    if anchor == head && head < len {
        (anchor, head + 1)
    } else {
        (anchor, head)
    }
}

/// Move head to the start of the selection without collapsing.
/// Anchor goes to the high end, head to the low end.
pub fn head_to_start(_text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let from = anchor.min(head);
    let to = anchor.max(head);
    (to, from)
}

/// Move head to the end of the selection without collapsing.
pub fn head_to_end(_text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let from = anchor.min(head);
    let to = anchor.max(head);
    (from, to)
}

/// Expand the range to cover the entire line (including trailing newline).
pub fn select_whole_line(text: &RopeSlice, _anchor: usize, head: usize) -> (usize, usize) {
    let line = text.char_to_line(head);
    let start = text.line_to_char(line);
    let end = if line + 1 < text.len_lines() {
        text.line_to_char(line + 1)
    } else {
        text.len_chars()
    };
    (start, end)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    fn rope(s: &str) -> Rope {
        Rope::from_str(s)
    }

    // ── char_class ───────────────────────────────────────────────────────

    #[test]
    fn classify_chars() {
        assert_eq!(char_class('\n'), CharClass::Eol);
        assert_eq!(char_class('\r'), CharClass::Eol);
        assert_eq!(char_class(' '), CharClass::Whitespace);
        assert_eq!(char_class('\t'), CharClass::Whitespace);
        assert_eq!(char_class('a'), CharClass::Word);
        assert_eq!(char_class('_'), CharClass::Word);
        assert_eq!(char_class('9'), CharClass::Word);
        assert_eq!(char_class('.'), CharClass::Punct);
        assert_eq!(char_class('('), CharClass::Punct);
    }

    // ── move_grapheme ───────────────────────────────────────────────────

    #[test]
    fn h_move_clamps_at_start() {
        let text = rope("hello");
        assert_eq!(move_grapheme(&text.slice(..), 0, -1), 0);
    }

    #[test]
    fn l_move_clamps_at_end() {
        let text = rope("hi");
        assert_eq!(move_grapheme(&text.slice(..), 2, 5), 2);
    }

    // ── move_vertical ────────────────────────────────────────────────────

    #[test]
    fn j_moves_down_preserving_col() {
        let text = rope("abc\nde\nfghij");
        assert_eq!(move_vertical(&text.slice(..), 2, 1, None), 5);
    }

    #[test]
    fn k_moves_up_preserving_col() {
        let text = rope("abcde\nfg");
        assert_eq!(move_vertical(&text.slice(..), 7, -1, None), 1);
    }

    #[test]
    fn sticky_col_remembers_wider_col() {
        let text = rope("abcde\nfg\nhijklm");
        let p1 = move_vertical(&text.slice(..), 4, 1, None);
        assert_eq!(p1, 7);
        let p2 = move_vertical(&text.slice(..), p1, 1, Some(4));
        assert_eq!(p2, 13);
    }

    #[test]
    fn k_at_first_line_stays_put() {
        let text = rope("hello\nworld");
        assert_eq!(move_vertical(&text.slice(..), 2, -1, None), 2);
    }

    #[test]
    fn j_at_last_line_stays_put() {
        let text = rope("hello\nworld");
        assert_eq!(move_vertical(&text.slice(..), 7, 1, None), 7);
    }

    #[test]
    fn j_does_not_descend_onto_trailing_newline_line() {
        // "abc\ndef\n" — ropey says 3 lines (last empty); vim says 2.
        // From line 1 col 0, j must stay on line 1, not jump to phantom line 2.
        let text = rope("abc\ndef\n");
        assert_eq!(move_vertical(&text.slice(..), 4, 1, None), 4);
    }

    #[test]
    fn j_from_phantom_trailing_line_clamps_back() {
        // If `pos` is on the phantom trailing line, j must not oscillate or
        // panic — it lands on the last navigable line, clamped to its last
        // visible char.
        let text = rope("abc\n");
        let pos = text.len_chars(); // = 4, on phantom line 1
        // last_navigable = 0; col carried from pos = 4, clamped to last
        // visible col of "abc" = 2.
        assert_eq!(move_vertical(&text.slice(..), pos, 1, None), 2);
    }

    #[test]
    fn last_navigable_line_handles_empty_and_trailing_newline() {
        assert_eq!(last_navigable_line(&rope("").slice(..)), 0);
        assert_eq!(last_navigable_line(&rope("abc").slice(..)), 0);
        assert_eq!(last_navigable_line(&rope("abc\n").slice(..)), 0);
        assert_eq!(last_navigable_line(&rope("abc\ndef").slice(..)), 1);
        assert_eq!(last_navigable_line(&rope("abc\ndef\n").slice(..)), 1);
        assert_eq!(last_navigable_line(&rope("abc\n\n").slice(..)), 1);
    }

    // ── skip_class ───────────────────────────────────────────────────────

    #[test]
    fn skip_class_fwd_word() {
        assert_eq!(skip_class_forward(&rope("hello.world").slice(..), 0), 5);
    }

    #[test]
    fn skip_class_fwd_punct() {
        assert_eq!(skip_class_forward(&rope("...abc").slice(..), 0), 3);
    }

    #[test]
    fn skip_class_fwd_at_end() {
        assert_eq!(skip_class_forward(&rope("hi").slice(..), 2), 2);
    }

    #[test]
    fn skip_class_bwd_word() {
        assert_eq!(skip_class_backward(&rope("hello.world").slice(..), 11), 6);
    }

    #[test]
    fn skip_class_bwd_punct() {
        assert_eq!(skip_class_backward(&rope("abc...").slice(..), 6), 3);
    }

    #[test]
    fn skip_class_bwd_at_start() {
        assert_eq!(skip_class_backward(&rope("hi").slice(..), 0), 0);
    }

    #[test]
    fn skip_class_fwd_whitespace() {
        assert_eq!(skip_class_forward(&rope("   abc").slice(..), 0), 3);
    }

    #[test]
    fn skip_class_fwd_whitespace_stops_at_eol() {
        assert_eq!(skip_class_forward(&rope("  \nabc").slice(..), 0), 2);
    }

    #[test]
    fn skip_class_bwd_whitespace() {
        assert_eq!(skip_class_backward(&rope("abc   def").slice(..), 6), 3);
    }

    #[test]
    fn skip_class_bwd_whitespace_stops_at_eol() {
        assert_eq!(skip_class_backward(&rope("abc\n  def").slice(..), 6), 4);
    }

    // ── find_char ───────────────────────────────────────────────────────

    #[test]
    fn find_char_fwd_found() {
        assert_eq!(find_char_forward(&rope("hello world").slice(..), 0, 'o'), 4);
    }

    #[test]
    fn find_char_fwd_not_found_stays() {
        assert_eq!(find_char_forward(&rope("hello").slice(..), 0, 'z'), 0);
    }

    #[test]
    fn find_char_fwd_stops_at_eol() {
        assert_eq!(find_char_forward(&rope("abc\ndef").slice(..), 0, 'e'), 0);
    }

    #[test]
    fn find_char_bwd_found() {
        assert_eq!(
            find_char_backward(&rope("hello world").slice(..), 10, 'o'),
            7
        );
    }

    #[test]
    fn find_char_bwd_not_found_stays() {
        assert_eq!(find_char_backward(&rope("hello").slice(..), 4, 'z'), 4);
    }

    #[test]
    fn find_char_bwd_stops_at_eol() {
        assert_eq!(find_char_backward(&rope("abc\ndef").slice(..), 6, 'a'), 6);
    }

    // ── match_bracket ───────────────────────────────────────────────────

    #[test]
    fn bracket_forward() {
        assert_eq!(match_bracket(&rope("(hello)").slice(..), 0), 6);
    }

    #[test]
    fn bracket_backward() {
        assert_eq!(match_bracket(&rope("(hello)").slice(..), 6), 0);
    }

    #[test]
    fn bracket_nested() {
        assert_eq!(match_bracket(&rope("(a(b)c)").slice(..), 0), 6);
    }

    #[test]
    fn bracket_no_match_stays() {
        assert_eq!(match_bracket(&rope("(hello").slice(..), 0), 0);
    }

    #[test]
    fn bracket_not_a_bracket_stays() {
        assert_eq!(match_bracket(&rope("hello").slice(..), 0), 0);
    }

    // ── line / doc motions ──────────────────────────────────────────────

    #[test]
    fn move_to_line_basic() {
        assert_eq!(move_to_line(&rope("abc\ndef\nghi").slice(..), 1), 4);
    }

    #[test]
    fn move_to_line_clamps() {
        assert_eq!(move_to_line(&rope("abc\ndef").slice(..), 100), 4);
    }

    #[test]
    fn zero_goes_to_line_start() {
        assert_eq!(move_line_start(&rope("abc\ndef").slice(..), 5), 4);
    }

    #[test]
    fn line_end_is_last_visible() {
        assert_eq!(move_line_end(&rope("abc\ndef").slice(..), 1), 2);
    }

    #[test]
    fn line_end_on_last_line_no_newline() {
        assert_eq!(move_line_end(&rope("abc").slice(..), 0), 2);
    }

    #[test]
    fn line_end_empty_line() {
        assert_eq!(move_line_end(&rope("abc\n\ndef").slice(..), 4), 4);
    }

    #[test]
    fn first_non_ws_skips_indent() {
        assert_eq!(
            move_first_non_whitespace(&rope("  \thello").slice(..), 0),
            3
        );
    }

    #[test]
    fn first_non_ws_no_indent() {
        assert_eq!(move_first_non_whitespace(&rope("hello").slice(..), 0), 0);
    }

    // ── paragraph motions ───────────────────────────────────────────────

    #[test]
    fn paragraph_forward_jumps_to_blank() {
        assert_eq!(move_paragraph_forward(&rope("aaa\n\nbbb").slice(..), 0), 4);
    }

    #[test]
    fn paragraph_backward_jumps_to_blank() {
        assert_eq!(move_paragraph_backward(&rope("aaa\n\nbbb").slice(..), 7), 4);
    }

    // ── selection-shape ────────────────────────────────────────────────

    #[test]
    fn select_whole_line_basic() {
        let (a, h) = select_whole_line(&rope("abc\ndef\n").slice(..), 0, 1);
        assert_eq!((a, h), (0, 4));
    }

    #[test]
    fn ensure_char_selected_extends_collapsed() {
        let (a, h) = ensure_char_selected(&rope("abc").slice(..), 0, 0);
        assert_eq!((a, h), (0, 1));
    }

    #[test]
    fn ensure_char_selected_leaves_non_collapsed() {
        let (a, h) = ensure_char_selected(&rope("abc").slice(..), 0, 2);
        assert_eq!((a, h), (0, 2));
    }
}
