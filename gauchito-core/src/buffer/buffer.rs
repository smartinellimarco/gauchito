use ropey::Rope;

use super::grapheme;
use crate::edit::Edit;

pub struct Buffer {
    content: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
        }
    }

    pub fn from_str(text: &str) -> Self {
        Self {
            content: Rope::from_str(text),
        }
    }

    pub fn text(&self) -> String {
        self.content.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.content.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.content.len_lines()
    }

    pub fn line(&self, idx: usize) -> String {
        self.content.line(idx).to_string()
    }

    pub fn line_len(&self, idx: usize) -> usize {
        self.content.line(idx).len_chars()
    }

    pub fn char_to_line(&self, pos: usize) -> usize {
        self.content.char_to_line(pos)
    }

    pub fn line_to_char(&self, line: usize) -> usize {
        self.content.line_to_char(line)
    }

    pub fn next_grapheme(&self, pos: usize) -> usize {
        grapheme::next_grapheme(&self.content.slice(..), pos)
    }

    pub fn prev_grapheme(&self, pos: usize) -> usize {
        grapheme::prev_grapheme(&self.content.slice(..), pos)
    }

    pub fn apply(&mut self, edit: &Edit) -> String {
        let replaced = self.content.slice(edit.start..edit.end).to_string();

        if edit.start != edit.end {
            self.content.remove(edit.start..edit.end);
        }
        if !edit.text.is_empty() {
            self.content.insert(edit.start, &edit.text);
        }

        replaced
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
