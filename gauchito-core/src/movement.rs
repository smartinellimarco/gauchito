use ropey::RopeSlice;

use crate::grapheme::{next_grapheme_boundary, prev_grapheme_boundary};

// ── Character classification ─────────────────────────────────────────────────

/// Character classification.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CharClass {
    Eol,
    Whitespace,
    Word,
    Punct,
}

/// Classify a character into one of the four classes.
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

// ── Bracket pairs ───────────────────────────────────────────────────────────

/// Matched bracket pairs: (open, close).
pub const BRACKET_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

/// Look up the matching bracket and direction for a character.
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

// ── Primitives ──────────────────────────────────────────────────────────────

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

    // Look at the char just before pos to determine the class
    let cls = char_class(text.char(pos - 1));

    let mut p = pos;
    while p > 0 && char_class(text.char(p - 1)) == cls {
        p -= 1;
    }

    p
}

/// Find the next occurrence of `ch` after `pos` on the same line.
pub fn find_char_forward(text: &RopeSlice, pos: usize, ch: char) -> Option<usize> {
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

/// Find the previous occurrence of `ch` before `pos` on the same line.
pub fn find_char_backward(text: &RopeSlice, pos: usize, ch: char) -> Option<usize> {
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

/// Find the matching bracket for the bracket at `pos`.
pub fn find_matching_bracket(text: &RopeSlice, pos: usize) -> Option<usize> {
    let len = text.len_chars();
    if pos >= len {
        return None;
    }

    let c = text.char(pos);
    let (target, forward) = bracket_pair(c)?;

    // Track nesting depth, starting at 1 for the bracket under cursor
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

// ── Conveniences ────────────────────────────────────────────────────────────
// Composed from primitives + ropey line queries.
// Kept here because the composition is non-trivial.

/// Step `count` lines, preserving column. Positive = down, negative = up.
pub fn move_vertical(
    text: &RopeSlice,
    pos: usize,
    count: isize,
    preferred_col: Option<usize>,
) -> usize {
    let last_line = text.len_lines().saturating_sub(1);
    let line = text.char_to_line(pos);

    // Sticky column: provided by caller across consecutive j/k presses,
    // otherwise derived from current position.
    let col = preferred_col.unwrap_or_else(|| pos.saturating_sub(text.line_to_char(line)));

    let new_line = if count >= 0 {
        (line + count as usize).min(last_line)
    } else {
        line.saturating_sub(count.unsigned_abs())
    };

    // Clamp column to visible chars on the target line.
    // saturating_sub(1): visible is a count, col is 0-indexed.
    let visible = visible_line_chars(text, new_line);
    let new_col = col.min(visible.saturating_sub(1));

    text.line_to_char(new_line) + new_col
}

/// First character on the line containing `pos`.
pub fn move_line_start(text: &RopeSlice, pos: usize) -> usize {
    text.line_to_char(text.char_to_line(pos))
}

/// Last visible character on the line containing `pos`.
pub fn move_line_end(text: &RopeSlice, pos: usize) -> usize {
    let line = text.char_to_line(pos);
    let line_start = text.line_to_char(line);
    let visible = visible_line_chars(text, line);

    if visible == 0 {
        line_start
    } else {
        // visible - 1: index of last char, not one past it
        line_start + visible - 1
    }
}

/// First non-whitespace character on the line containing `pos`.
pub fn move_first_non_whitespace(text: &RopeSlice, pos: usize) -> usize {
    let line = text.char_to_line(pos);
    let line_start = text.line_to_char(line);

    let offset = text
        .line(line)
        .chars()
        .take_while(|&c| char_class(c) == CharClass::Whitespace)
        .count();

    line_start + offset
}

/// First character of line `line` (0-indexed), clamped to document bounds.
pub fn move_to_line(text: &RopeSlice, line: usize) -> usize {
    let last_line = text.len_lines().saturating_sub(1);

    text.line_to_char(line.min(last_line))
}

/// Count visible (non-Eol) characters on a given line.
fn visible_line_chars(text: &RopeSlice, line: usize) -> usize {
    text.line(line)
        .chars()
        .take_while(|&c| char_class(c) != CharClass::Eol)
        .count()
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

    // ── skip_class_forward ───────────────────────────────────────────────

    #[test]
    fn skip_class_fwd_word() {
        let text = rope("hello.world");
        assert_eq!(skip_class_forward(&text.slice(..), 0), 5);
    }

    #[test]
    fn skip_class_fwd_punct() {
        let text = rope("...abc");
        assert_eq!(skip_class_forward(&text.slice(..), 0), 3);
    }

    #[test]
    fn skip_class_fwd_at_end() {
        let text = rope("hi");
        assert_eq!(skip_class_forward(&text.slice(..), 2), 2);
    }

    // ── skip_class_backward ──────────────────────────────────────────────

    #[test]
    fn skip_class_bwd_word() {
        let text = rope("hello.world");
        assert_eq!(skip_class_backward(&text.slice(..), 11), 6);
    }

    #[test]
    fn skip_class_bwd_punct() {
        let text = rope("abc...");
        assert_eq!(skip_class_backward(&text.slice(..), 6), 3);
    }

    #[test]
    fn skip_class_bwd_at_start() {
        assert_eq!(skip_class_backward(&rope("hi").slice(..), 0), 0);
    }

    // skip_class on whitespace stops at Eol (Whitespace and Eol are different classes)

    #[test]
    fn skip_class_fwd_whitespace() {
        let text = rope("   abc");
        assert_eq!(skip_class_forward(&text.slice(..), 0), 3);
    }

    #[test]
    fn skip_class_fwd_whitespace_stops_at_eol() {
        let text = rope("  \nabc");
        assert_eq!(skip_class_forward(&text.slice(..), 0), 2);
    }

    #[test]
    fn skip_class_bwd_whitespace() {
        let text = rope("abc   def");
        assert_eq!(skip_class_backward(&text.slice(..), 6), 3);
    }

    #[test]
    fn skip_class_bwd_whitespace_stops_at_eol() {
        let text = rope("abc\n  def");
        assert_eq!(skip_class_backward(&text.slice(..), 6), 4);
    }

    // ── find_char_forward / backward ────────────────────────────────────

    #[test]
    fn find_char_fwd_found() {
        let text = rope("hello world");
        assert_eq!(find_char_forward(&text.slice(..), 0, 'o'), Some(4));
    }

    #[test]
    fn find_char_fwd_not_found() {
        let text = rope("hello");
        assert_eq!(find_char_forward(&text.slice(..), 0, 'z'), None);
    }

    #[test]
    fn find_char_fwd_stops_at_eol() {
        let text = rope("abc\ndef");
        assert_eq!(find_char_forward(&text.slice(..), 0, 'e'), None);
    }

    #[test]
    fn find_char_bwd_found() {
        let text = rope("hello world");
        assert_eq!(find_char_backward(&text.slice(..), 10, 'o'), Some(7));
    }

    #[test]
    fn find_char_bwd_not_found() {
        let text = rope("hello");
        assert_eq!(find_char_backward(&text.slice(..), 4, 'z'), None);
    }

    #[test]
    fn find_char_bwd_stops_at_eol() {
        let text = rope("abc\ndef");
        assert_eq!(find_char_backward(&text.slice(..), 6, 'a'), None);
    }

    // ── find_matching_bracket ───────────────────────────────────────────

    #[test]
    fn bracket_forward() {
        let text = rope("(hello)");
        assert_eq!(find_matching_bracket(&text.slice(..), 0), Some(6));
    }

    #[test]
    fn bracket_backward() {
        let text = rope("(hello)");
        assert_eq!(find_matching_bracket(&text.slice(..), 6), Some(0));
    }

    #[test]
    fn bracket_nested() {
        let text = rope("(a(b)c)");
        assert_eq!(find_matching_bracket(&text.slice(..), 0), Some(6));
    }

    #[test]
    fn bracket_no_match() {
        let text = rope("(hello");
        assert_eq!(find_matching_bracket(&text.slice(..), 0), None);
    }

    #[test]
    fn bracket_not_a_bracket() {
        let text = rope("hello");
        assert_eq!(find_matching_bracket(&text.slice(..), 0), None);
    }

    // ── move_to_line ────────────────────────────────────────────────────

    #[test]
    fn move_to_line_basic() {
        let text = rope("abc\ndef\nghi");
        assert_eq!(move_to_line(&text.slice(..), 1), 4);
    }

    #[test]
    fn move_to_line_clamps() {
        let text = rope("abc\ndef");
        assert_eq!(move_to_line(&text.slice(..), 100), 4);
    }

    // ── move_line_start / end ────────────────────────────────────────────

    #[test]
    fn zero_goes_to_line_start() {
        let text = rope("abc\ndef");
        assert_eq!(move_line_start(&text.slice(..), 5), 4);
    }

    #[test]
    fn dollar_goes_to_last_char_before_newline() {
        let text = rope("abc\ndef");
        assert_eq!(move_line_end(&text.slice(..), 1), 2);
    }

    #[test]
    fn dollar_on_last_line_no_newline() {
        let text = rope("abc");
        assert_eq!(move_line_end(&text.slice(..), 0), 2);
    }

    // ── move_first_non_whitespace ────────────────────────────────────────

    #[test]
    fn first_non_ws_skips_indent() {
        let text = rope("  \thello");
        assert_eq!(move_first_non_whitespace(&text.slice(..), 0), 3);
    }

    #[test]
    fn first_non_ws_no_indent() {
        let text = rope("hello");
        assert_eq!(move_first_non_whitespace(&text.slice(..), 0), 0);
    }
}
