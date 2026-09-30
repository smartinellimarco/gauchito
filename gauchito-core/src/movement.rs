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

fn last_visible(text: &RopeSlice, line: usize) -> usize {
    let start = line_start(text, line);
    let end = start + visible_chars(text, line).map(char::len_utf8).sum::<usize>();
    if end == start {
        start
    } else {
        text.floor_char_boundary(end - 1)
    }
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
    text.slice(line_start(text, line_of(text, pos))..pos)
        .chars()
        .count()
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

#[cfg(test)]
mod tests {
    use crate::movement::{
        CharClass, char_class, last_navigable_line, move_first_non_whitespace, move_grapheme,
        move_line_end, move_line_start, move_to_line, move_vertical, position_at,
        skip_class_backward, skip_class_forward, visual_column,
    };
    use ropey::Rope;

    fn rope(s: &str) -> Rope {
        Rope::from_str(s)
    }

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
    fn j_descends_onto_trailing_newline_line() {
        let text = rope("abc\ndef\n");
        let dest = move_vertical(&text.slice(..), 4, 1, None);
        assert_eq!(dest, 8);
    }

    #[test]
    fn j_from_trailing_line_stays_put() {
        let text = rope("abc\n");
        let pos = text.len();
        assert_eq!(move_vertical(&text.slice(..), pos, 1, None), pos);
    }

    #[test]
    fn last_navigable_line_handles_empty_and_trailing_newline() {
        assert_eq!(last_navigable_line(&rope("").slice(..)), 0);
        assert_eq!(last_navigable_line(&rope("abc").slice(..)), 0);
        assert_eq!(last_navigable_line(&rope("abc\n").slice(..)), 1);
        assert_eq!(last_navigable_line(&rope("abc\ndef").slice(..)), 1);
        assert_eq!(last_navigable_line(&rope("abc\ndef\n").slice(..)), 2);
        assert_eq!(last_navigable_line(&rope("abc\n\n").slice(..)), 2);
    }

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

    #[test]
    fn grapheme_moves_step_over_multibyte_clusters() {
        let text = rope("añ👍🏽b");
        let t = text.slice(..);
        assert_eq!(move_grapheme(&t, 1, 1), 3);
        assert_eq!(move_grapheme(&t, 3, 1), 11);
        assert_eq!(move_grapheme(&t, 11, -1), 3);
        assert_eq!(move_grapheme(&t, 11, -2), 1);
    }

    #[test]
    fn grapheme_moves_on_empty_rope_stay_at_zero() {
        let text = rope("");
        assert_eq!(move_grapheme(&text.slice(..), 0, 1), 0);
        assert_eq!(move_grapheme(&text.slice(..), 0, -1), 0);
    }

    #[test]
    fn class_skips_count_bytes() {
        let text = rope("ñandú ok");
        let t = text.slice(..);
        assert_eq!(skip_class_forward(&t, 0), 7);
        assert_eq!(skip_class_backward(&t, 7), 0);
    }

    #[test]
    fn columns_count_chars_and_positions_count_bytes() {
        let text = rope(
            "ññññ
    abcd",
        );
        let t = text.slice(..);
        assert_eq!(visual_column(&t, 6), 3);
        assert_eq!(position_at(&t, 0, 2), 4);
        assert_eq!(move_vertical(&t, 11, -1, None), 4);
        assert_eq!(move_line_end(&t, 0), 6);
    }
}
