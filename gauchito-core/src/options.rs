//! [`PartialBufferOptions`] — sparse overlay for layered configuration.
//!
//! Layers feed [`BufferOptions`] at open time, in increasing
//! priority: substrate defaults → sniffed-on-disk → distro overrides. Each layer produces a partial;
//! [`PartialBufferOptions::merge`] is later-wins for `Some` fields,
//! [`PartialBufferOptions::resolve`] fills the remaining `None`s
//! with a baseline.
//!
//! One shape for every layer keeps the conversions out of the
//! callers' hair — no per-layer struct, no boolean flags for "is
//! this field set."

use crate::buffer::{BufferOptions, IndentStyle, LineEnding};

#[derive(Debug, Default, Clone)]
pub struct PartialBufferOptions {
    pub line_ending:              Option<LineEnding>,
    pub final_newline:            Option<bool>,
    pub bom:                      Option<bool>,
    pub trim_trailing_whitespace: Option<bool>,
    pub indent:                   Option<IndentStyle>,
}

impl PartialBufferOptions {
    pub fn merge(mut self, other: PartialBufferOptions) -> Self {
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

    pub fn resolve(self, base: BufferOptions) -> BufferOptions {
        BufferOptions {
            line_ending:              self.line_ending.unwrap_or(base.line_ending),
            final_newline:            self.final_newline.unwrap_or(base.final_newline),
            bom:                      self.bom.unwrap_or(base.bom),
            trim_trailing_whitespace: self
                .trim_trailing_whitespace
                .unwrap_or(base.trim_trailing_whitespace),
            indent:                   self.indent.unwrap_or(base.indent),
        }
    }
}
