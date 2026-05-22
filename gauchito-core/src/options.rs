use crate::document::{DocumentOptions, IndentStyle, LineEnding};

#[derive(Debug, Default, Clone)]
pub struct PartialDocumentOptions {
    pub line_ending: Option<LineEnding>,
    pub final_newline: Option<bool>,
    pub bom: Option<bool>,
    pub trim_trailing_whitespace: Option<bool>,
    pub indent: Option<IndentStyle>,
}

impl PartialDocumentOptions {
    pub fn merge(mut self, other: PartialDocumentOptions) -> Self {
        if other.line_ending.is_some() {
            self.line_ending = other.line_ending;
        }
        if other.final_newline.is_some() {
            self.final_newline = other.final_newline;
        }
        if other.bom.is_some() {
            self.bom = other.bom;
        }
        if other.trim_trailing_whitespace.is_some() {
            self.trim_trailing_whitespace = other.trim_trailing_whitespace;
        }
        if other.indent.is_some() {
            self.indent = other.indent;
        }
        self
    }

    pub fn resolve(self, base: DocumentOptions) -> DocumentOptions {
        DocumentOptions {
            line_ending: self.line_ending.unwrap_or(base.line_ending),
            final_newline: self.final_newline.unwrap_or(base.final_newline),
            bom: self.bom.unwrap_or(base.bom),
            trim_trailing_whitespace: self
                .trim_trailing_whitespace
                .unwrap_or(base.trim_trailing_whitespace),
            indent: self.indent.unwrap_or(base.indent),
        }
    }
}
