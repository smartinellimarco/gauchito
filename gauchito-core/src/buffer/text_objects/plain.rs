use super::{SelectionMode, TextObjectMatcher, TextSource};
use std::ops::Range;

///// Find the previous grapheme boundary before the given char position
//pub fn prev_grapheme_boundary(&self, char_idx: usize) -> usize {
//    self.prev_grapheme_boundary_slice(&self.content.slice(..), char_idx)
//}
//
///// Find the next grapheme boundary after the given char position
//pub fn next_grapheme_boundary(&self, char_idx: usize) -> usize {
//    self.next_grapheme_boundary_slice(&self.content.slice(..), char_idx)
//}
//
///// Check if the given char position is a grapheme boundary
//pub fn is_grapheme_boundary(&self, char_idx: usize) -> bool {
//    self.is_grapheme_boundary_slice(&self.content.slice(..), char_idx)
//}
//
//// Helper methods that work on slices (for reuse in text objects)
//fn prev_grapheme_boundary_slice(&self, slice: &RopeSlice, char_idx: usize) -> usize {
//    // Bounds check
//    if char_idx == 0 || char_idx > slice.len_chars() {
//        return 0;
//    }
//
//    // We work with bytes for this, so convert.
//    let byte_idx = slice.char_to_byte(char_idx);
//
//    // Get the chunk with our byte index in it.
//    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);
//
//    // Set up the grapheme cursor.
//    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
//
//    // Find the previous grapheme cluster boundary.
//    loop {
//        match gc.prev_boundary(chunk, chunk_byte_idx) {
//            Ok(None) => return 0,
//            Ok(Some(n)) => {
//                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
//                return chunk_char_idx + tmp;
//            }
//            Err(GraphemeIncomplete::PrevChunk) => {
//                if chunk_byte_idx == 0 {
//                    return 0;
//                }
//                let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
//                chunk = a;
//                chunk_byte_idx = b;
//                chunk_char_idx = c;
//            }
//            Err(GraphemeIncomplete::PreContext(n)) => {
//                let ctx_chunk = slice.chunk_at_byte(n.saturating_sub(1)).0;
//                gc.provide_context(ctx_chunk, n.saturating_sub(ctx_chunk.len()));
//            }
//            _ => unreachable!(),
//        }
//    }
//}
//
//fn next_grapheme_boundary_slice(&self, slice: &RopeSlice, char_idx: usize) -> usize {
//    // Bounds check
//    if char_idx >= slice.len_chars() {
//        return slice.len_chars();
//    }
//
//    // We work with bytes for this, so convert.
//    let byte_idx = slice.char_to_byte(char_idx);
//
//    // Get the chunk with our byte index in it.
//    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);
//
//    // Set up the grapheme cursor.
//    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
//
//    // Find the next grapheme cluster boundary.
//    loop {
//        match gc.next_boundary(chunk, chunk_byte_idx) {
//            Ok(None) => return slice.len_chars(),
//            Ok(Some(n)) => {
//                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
//                return chunk_char_idx + tmp;
//            }
//            Err(GraphemeIncomplete::NextChunk) => {
//                chunk_byte_idx += chunk.len();
//                let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
//                chunk = a;
//                chunk_char_idx = c;
//            }
//            Err(GraphemeIncomplete::PreContext(n)) => {
//                let ctx_chunk = slice.chunk_at_byte(n.saturating_sub(1)).0;
//                gc.provide_context(ctx_chunk, n.saturating_sub(ctx_chunk.len()));
//            }
//            _ => unreachable!(),
//        }
//    }
//}
//
//fn is_grapheme_boundary_slice(&self, slice: &RopeSlice, char_idx: usize) -> bool {
//    // Bounds check
//    if char_idx > slice.len_chars() {
//        return false;
//    }
//    if char_idx == 0 || char_idx == slice.len_chars() {
//        return true;
//    }
//
//    // We work with bytes for this, so convert.
//    let byte_idx = slice.char_to_byte(char_idx);
//
//    // Get the chunk with our byte index in it.
//    let (chunk, chunk_byte_idx, _, _) = slice.chunk_at_byte(byte_idx);
//
//    // Set up the grapheme cursor.
//    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
//
//    // Determine if the given position is a grapheme cluster boundary.
//    loop {
//        match gc.is_boundary(chunk, chunk_byte_idx) {
//            Ok(n) => return n,
//            Err(GraphemeIncomplete::PreContext(n)) => {
//                let (ctx_chunk, ctx_byte_start, _, _) = slice.chunk_at_byte(n.saturating_sub(1));
//                gc.provide_context(ctx_chunk, ctx_byte_start);
//            }
//            _ => unreachable!(),
//        }
//    }
//}
#[derive(Debug)]
pub struct WordMatcher;

impl TextObjectMatcher for WordMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        if pos >= buffer.len_chars() {
            return None;
        }

        let ch = buffer.char_at(pos)?;
        if !is_word_char(ch) {
            return None;
        }

        let mut start = pos;
        while start > 0 {
            if let Some(ch) = buffer.char_at(start - 1) {
                if !is_word_char(ch) {
                    break;
                }
                start -= 1;
            } else {
                break;
            }
        }

        let mut end = pos + 1;
        while end < buffer.len_chars() {
            if let Some(ch) = buffer.char_at(end) {
                if !is_word_char(ch) {
                    break;
                }
                end += 1;
            } else {
                break;
            }
        }

        match mode {
            SelectionMode::Inside => Some(start..end),
            SelectionMode::Around => {
                // Include trailing whitespace
                while end < buffer.len_chars() {
                    if let Some(ch) = buffer.char_at(end) {
                        if !ch.is_whitespace() {
                            break;
                        }
                        end += 1;
                    } else {
                        break;
                    }
                }
                Some(start..end)
            }
        }
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let mut search_pos = pos + 1;
        while search_pos < buffer.len_chars() {
            if let Some(ch) = buffer.char_at(search_pos) {
                if is_word_char(ch) {
                    return self.find_at(buffer, search_pos, mode);
                }
                search_pos += 1;
            } else {
                break;
            }
        }
        None
    }

    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        if pos == 0 {
            return None;
        }

        let mut search_pos = pos.saturating_sub(1);
        loop {
            if let Some(ch) = buffer.char_at(search_pos) {
                if is_word_char(ch) {
                    return self.find_at(buffer, search_pos, mode);
                }
                if search_pos == 0 {
                    break;
                }
                search_pos -= 1;
            } else {
                break;
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct BigWordMatcher;

impl TextObjectMatcher for BigWordMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        if pos >= buffer.len_chars() {
            return None;
        }

        let ch = buffer.char_at(pos)?;
        if ch.is_whitespace() {
            return None;
        }

        let mut start = pos;
        while start > 0 {
            if let Some(ch) = buffer.char_at(start - 1) {
                if ch.is_whitespace() {
                    break;
                }
                start -= 1;
            } else {
                break;
            }
        }

        let mut end = pos + 1;
        while end < buffer.len_chars() {
            if let Some(ch) = buffer.char_at(end) {
                if ch.is_whitespace() {
                    break;
                }
                end += 1;
            } else {
                break;
            }
        }

        match mode {
            SelectionMode::Inside => Some(start..end),
            SelectionMode::Around => {
                while end < buffer.len_chars() {
                    if let Some(ch) = buffer.char_at(end) {
                        if !ch.is_whitespace() {
                            break;
                        }
                        end += 1;
                    } else {
                        break;
                    }
                }
                Some(start..end)
            }
        }
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let mut search_pos = pos + 1;
        while search_pos < buffer.len_chars() {
            if let Some(ch) = buffer.char_at(search_pos) {
                if !ch.is_whitespace() {
                    return self.find_at(buffer, search_pos, mode);
                }
                search_pos += 1;
            } else {
                break;
            }
        }
        None
    }

    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        if pos == 0 {
            return None;
        }

        let mut search_pos = pos.saturating_sub(1);
        loop {
            if let Some(ch) = buffer.char_at(search_pos) {
                if !ch.is_whitespace() {
                    return self.find_at(buffer, search_pos, mode);
                }
                if search_pos == 0 {
                    break;
                }
                search_pos -= 1;
            } else {
                break;
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct ParagraphMatcher;

impl TextObjectMatcher for ParagraphMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        _mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let current_line = buffer.char_to_line(pos);

        // Find start of paragraph (first non-blank line above blank lines)
        let mut start_line = current_line;
        while start_line > 0 {
            let line_start = buffer.line_to_char(start_line);
            let line_end = if start_line + 1 < buffer.len_lines() {
                buffer.line_to_char(start_line + 1)
            } else {
                buffer.len_chars()
            };

            let is_blank = (line_start..line_end).all(|i| {
                buffer
                    .char_at(i)
                    .map(|ch| ch.is_whitespace())
                    .unwrap_or(true)
            });

            if is_blank {
                start_line += 1;
                break;
            }
            start_line -= 1;
        }

        // Find end of paragraph
        let mut end_line = current_line;
        while end_line < buffer.len_lines() {
            let line_start = buffer.line_to_char(end_line);
            let line_end = if end_line + 1 < buffer.len_lines() {
                buffer.line_to_char(end_line + 1)
            } else {
                buffer.len_chars()
            };

            let is_blank = (line_start..line_end).all(|i| {
                buffer
                    .char_at(i)
                    .map(|ch| ch.is_whitespace())
                    .unwrap_or(true)
            });

            if is_blank {
                break;
            }
            end_line += 1;
        }

        let start = buffer.line_to_char(start_line);
        let end = buffer.line_to_char(end_line);

        Some(start..end)
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let current_line = buffer.char_to_line(pos);
        let mut search_line = current_line + 1;

        while search_line < buffer.len_lines() {
            let line_start = buffer.line_to_char(search_line);
            self.find_at(buffer, line_start, mode)?;
            search_line += 1;
        }
        None
    }

    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let current_line = buffer.char_to_line(pos);
        if current_line == 0 {
            return None;
        }

        let search_line = current_line - 1;
        let line_start = buffer.line_to_char(search_line);
        self.find_at(buffer, line_start, mode)
    }
}

#[derive(Debug)]
pub struct DelimiterMatcher {
    open: char,
    close: char,
}

impl DelimiterMatcher {
    pub fn new(open: char, close: char) -> Self {
        Self { open, close }
    }

    pub fn parentheses() -> Self {
        Self::new('(', ')')
    }

    pub fn brackets() -> Self {
        Self::new('[', ']')
    }

    pub fn braces() -> Self {
        Self::new('{', '}')
    }

    pub fn angle_brackets() -> Self {
        Self::new('<', '>')
    }
}

impl TextObjectMatcher for DelimiterMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        // Find enclosing delimiters
        let mut depth = 0;
        let mut start = None;

        // Search backward for opening delimiter
        for i in (0..=pos).rev() {
            let ch = buffer.char_at(i)?;
            if ch == self.close {
                depth += 1;
            } else if ch == self.open {
                if depth == 0 {
                    start = Some(i);
                    break;
                }
                depth -= 1;
            }
        }

        let start_pos = start?;

        // Search forward for closing delimiter
        depth = 0;
        for i in start_pos..buffer.len_chars() {
            let ch = buffer.char_at(i)?;
            if ch == self.open {
                depth += 1;
            } else if ch == self.close {
                depth -= 1;
                if depth == 0 {
                    return match mode {
                        SelectionMode::Inside => Some(start_pos + 1..i),
                        SelectionMode::Around => Some(start_pos..i + 1),
                    };
                }
            }
        }

        None
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        for i in pos + 1..buffer.len_chars() {
            if buffer.char_at(i)? == self.open {
                return self.find_at(buffer, i + 1, mode);
            }
        }
        None
    }

    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        for i in (0..pos).rev() {
            if buffer.char_at(i)? == self.open {
                return self.find_at(buffer, i + 1, mode);
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct QuoteMatcher {
    quote: char,
}

impl QuoteMatcher {
    pub fn new(quote: char) -> Self {
        Self { quote }
    }

    pub fn single() -> Self {
        Self::new('\'')
    }

    pub fn double() -> Self {
        Self::new('"')
    }

    pub fn backtick() -> Self {
        Self::new('`')
    }
}

impl TextObjectMatcher for QuoteMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        // Find enclosing quotes
        let mut start = None;
        let mut escaped = false;

        // Search backward
        for i in (0..=pos).rev() {
            let ch = buffer.char_at(i)?;
            if ch == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if ch == self.quote && !escaped {
                start = Some(i);
                break;
            }
            escaped = false;
        }

        let start_pos = start?;

        // Search forward
        escaped = false;
        for i in start_pos + 1..buffer.len_chars() {
            let ch = buffer.char_at(i)?;
            if ch == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if ch == self.quote && !escaped {
                return match mode {
                    SelectionMode::Inside => Some(start_pos + 1..i),
                    SelectionMode::Around => Some(start_pos..i + 1),
                };
            }
            escaped = false;
        }

        None
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let mut escaped = false;
        for i in pos + 1..buffer.len_chars() {
            let ch = buffer.char_at(i)?;
            if ch == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if ch == self.quote && !escaped {
                return self.find_at(buffer, i + 1, mode);
            }
            escaped = false;
        }
        None
    }

    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let mut escaped = false;
        for i in (0..pos).rev() {
            let ch = buffer.char_at(i)?;
            if ch == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if ch == self.quote && !escaped {
                return self.find_at(buffer, i + 1, mode);
            }
            escaped = false;
        }
        None
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}
