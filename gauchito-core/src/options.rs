/// Line ending style for a file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        }
    }
}

/// Resolved file-format options stored on `Document`, applied at save time.
#[derive(Debug, Clone)]
pub struct DocumentOptions {
    pub line_ending: LineEnding,
    pub final_newline: bool,
    pub bom: bool,
    pub trim_trailing_whitespace: bool,
}

impl Default for DocumentOptions {
    fn default() -> Self {
        Self {
            line_ending: LineEnding::Lf,
            final_newline: true,
            bom: false,
            trim_trailing_whitespace: false,
        }
    }
}

// TODO: this should be a responsability of file IO
impl DocumentOptions {
    /// Merge sniffed file properties with overrides.
    /// Overrides take precedence; anything unspecified falls back to what was observed.
    pub(crate) fn resolve(
        line_ending: LineEnding,
        final_newline: bool,
        bom: bool,
        override_line_ending: Option<LineEnding>,
        override_final_newline: Option<bool>,
        override_trim_trailing_ws: Option<bool>,
    ) -> Self {
        Self {
            line_ending: override_line_ending.unwrap_or(line_ending),
            final_newline: override_final_newline.unwrap_or(final_newline),
            bom,
            trim_trailing_whitespace: override_trim_trailing_ws.unwrap_or(false),
        }
    }
}
