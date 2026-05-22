use ropey::RopeSlice;

use crate::grapheme::{next_grapheme_boundary, prev_grapheme_boundary};

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

pub const BRACKET_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

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

pub fn move_vertical(
    text: &RopeSlice,
    pos: usize,
    count: isize,
    preferred_col: Option<usize>,
) -> usize {
    let last_line = last_navigable_line(text);
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

pub fn move_left(text: &RopeSlice, head: usize) -> usize {
    move_grapheme(text, head, -1)
}

pub fn move_right(text: &RopeSlice, head: usize) -> usize {
    move_grapheme(text, head, 1)
}

pub fn move_left_inline(text: &RopeSlice, head: usize) -> usize {
    let line = text.char_to_line(head);
    let line_start = text.line_to_char(line);
    if head <= line_start {
        head
    } else {
        move_grapheme(text, head, -1)
    }
}

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

pub fn move_line_start(text: &RopeSlice, head: usize) -> usize {
    text.line_to_char(text.char_to_line(head))
}

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

pub fn move_doc_start(_text: &RopeSlice, _head: usize) -> usize {
    0
}

pub fn move_doc_end(text: &RopeSlice, _head: usize) -> usize {
    text.len_chars()
}

pub fn move_to_line(text: &RopeSlice, line: usize) -> usize {
    let last_line = text.len_lines().saturating_sub(1);
    text.line_to_char(line.min(last_line))
}

pub fn visible_line_chars(text: &RopeSlice, line: usize) -> usize {
    text.line(line)
        .chars()
        .take_while(|&c| char_class(c) != CharClass::Eol)
        .count()
}

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

pub fn ensure_char_selected(text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let len = text.len_chars();
    if anchor == head && head < len {
        (anchor, head + 1)
    } else {
        (anchor, head)
    }
}

pub fn head_to_start(_text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let from = anchor.min(head);
    let to = anchor.max(head);
    (to, from)
}

pub fn head_to_end(_text: &RopeSlice, anchor: usize, head: usize) -> (usize, usize) {
    let from = anchor.min(head);
    let to = anchor.max(head);
    (from, to)
}

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
