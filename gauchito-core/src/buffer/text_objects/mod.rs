pub mod kinds;
pub mod plain;
pub mod regex;
pub mod treesitter;

use std::ops::Range;

/// Selection mode for text objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Inside,
    Around,
}

/// Text object matcher trait
pub trait TextObjectMatcher: std::fmt::Debug + Send + Sync {
    /// Find text object at or containing the given position
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>>;

    /// Find next occurrence of text object from position
    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>>;

    /// Find previous occurrence of text object from position
    fn find_prev(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>>;
}

/// Abstraction over text sources (Buffer's Rope without exposing it)
pub trait TextSource {
    fn len_chars(&self) -> usize;
    fn len_lines(&self) -> usize;
    fn char_at(&self, pos: usize) -> Option<char>;
    fn char_to_line(&self, pos: usize) -> usize;
    fn line_to_char(&self, line: usize) -> usize;
    fn slice_to_string(&self, start: usize, end: usize) -> String;
    fn line_chars(&self, line: usize) -> Box<dyn Iterator<Item = char> + '_>;

    // Grapheme support
    fn prev_grapheme_boundary(&self, pos: usize) -> usize;
    fn next_grapheme_boundary(&self, pos: usize) -> usize;
    fn is_grapheme_boundary(&self, pos: usize) -> bool;
}

// Implement TextSource for Buffer
impl TextSource for crate::buffer::Buffer {
    fn len_chars(&self) -> usize {
        self.len_chars()
    }

    fn len_lines(&self) -> usize {
        self.len_lines()
    }

    fn char_at(&self, pos: usize) -> Option<char> {
        self.char_at(pos)
    }

    fn char_to_line(&self, pos: usize) -> usize {
        self.char_to_line(pos)
    }

    fn line_to_char(&self, line: usize) -> usize {
        self.line_to_char(line)
    }

    fn slice_to_string(&self, start: usize, end: usize) -> String {
        use crate::query_engines::traits::TextNavigator;
        self.slice_to_string(start, end)
    }

    fn line_chars(&self, line: usize) -> Box<dyn Iterator<Item = char> + '_> {
        use crate::query_engines::traits::TextNavigator;
        self.line_chars(line)
    }

    fn prev_grapheme_boundary(&self, pos: usize) -> usize {
        self.prev_grapheme_boundary(pos)
    }

    fn next_grapheme_boundary(&self, pos: usize) -> usize {
        self.next_grapheme_boundary(pos)
    }

    fn is_grapheme_boundary(&self, pos: usize) -> bool {
        self.is_grapheme_boundary(pos)
    }
}
