use super::{SelectionMode, TextObjectMatcher, TextSource};
use regex::Regex;
use std::ops::Range;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct RegexMatcher {
    pattern: &'static Regex,
}

impl RegexMatcher {
    pub fn url() -> Self {
        static URL_REGEX: OnceLock<Regex> = OnceLock::new();
        let pattern =
            URL_REGEX.get_or_init(|| Regex::new(r#"https?://[^\s<>"']+|www\.[^\s<>"']+"#).unwrap());
        Self { pattern }
    }

    pub fn email() -> Self {
        static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
        let pattern = EMAIL_REGEX.get_or_init(|| {
            Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap()
        });
        Self { pattern }
    }

    pub fn number() -> Self {
        static NUMBER_REGEX: OnceLock<Regex> = OnceLock::new();
        let pattern = NUMBER_REGEX.get_or_init(|| Regex::new(r"-?\d+\.?\d*").unwrap());
        Self { pattern }
    }

    pub fn hex_color() -> Self {
        static HEX_REGEX: OnceLock<Regex> = OnceLock::new();
        let pattern =
            HEX_REGEX.get_or_init(|| Regex::new(r"#[0-9a-fA-F]{6}|#[0-9a-fA-F]{3}").unwrap());
        Self { pattern }
    }

    fn find_in_text(&self, text: &str, offset: usize, mode: SelectionMode) -> Option<Range<usize>> {
        // Check if there's a match at the current position
        for mat in self.pattern.find_iter(text) {
            let start = mat.start();
            let end = mat.end();

            if start <= offset && offset < end {
                return Some(start..end);
            }
        }
        None
    }
}

impl TextObjectMatcher for RegexMatcher {
    fn find_at(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        // Get the current line
        let line_idx = buffer.char_to_line(pos);
        let line_start = buffer.line_to_char(line_idx);
        let line_text = buffer.slice_to_string(
            line_start,
            if line_idx + 1 < buffer.len_lines() {
                buffer.line_to_char(line_idx + 1)
            } else {
                buffer.len_chars()
            },
        );

        let offset = pos - line_start;
        let range = self.find_in_text(&line_text, offset, mode)?;

        Some(line_start + range.start..line_start + range.end)
    }

    fn find_next(
        &self,
        buffer: &dyn TextSource,
        pos: usize,
        mode: SelectionMode,
    ) -> Option<Range<usize>> {
        let start_line = buffer.char_to_line(pos);

        for line_idx in start_line..buffer.len_lines() {
            let line_start = buffer.line_to_char(line_idx);
            let line_end = if line_idx + 1 < buffer.len_lines() {
                buffer.line_to_char(line_idx + 1)
            } else {
                buffer.len_chars()
            };

            let line_text = buffer.slice_to_string(line_start, line_end);
            let search_from = if line_idx == start_line {
                pos - line_start + 1
            } else {
                0
            };

            if let Some(mat) = self.pattern.find_at(&line_text, search_from) {
                return Some(line_start + mat.start()..line_start + mat.end());
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
        let start_line = buffer.char_to_line(pos);

        for line_idx in (0..=start_line).rev() {
            let line_start = buffer.line_to_char(line_idx);
            let line_end = if line_idx + 1 < buffer.len_lines() {
                buffer.line_to_char(line_idx + 1)
            } else {
                buffer.len_chars()
            };

            let line_text = buffer.slice_to_string(line_start, line_end);

            // Find all matches in the line
            let matches: Vec<_> = self.pattern.find_iter(&line_text).collect();

            for mat in matches.into_iter().rev() {
                let abs_start = line_start + mat.start();
                if abs_start < pos {
                    return Some(abs_start..line_start + mat.end());
                }
            }
        }

        None
    }
}
