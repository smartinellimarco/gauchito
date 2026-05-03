// TODO: todo el flujo de lang config / editorconfig / infer / standarization on load no se hace
use std::path::Path;

use ec4rs::property::{EndOfLine, FinalNewline, TrimTrailingWs};
use crate::options::LineEnding;

/// Overrides from `.editorconfig`. `None` means "not specified, use sniffed value".
#[derive(Debug, Default)]
pub struct EditorConfigRules {
    pub line_ending: Option<LineEnding>,
    pub final_newline: Option<bool>,
    pub trim_trailing_whitespace: Option<bool>,
}

/// Read `.editorconfig` files for the given path and convert to our rules type.
pub fn rules_for(path: &Path) -> EditorConfigRules {
    let props = match ec4rs::properties_of(path) {
        Ok(p) => p,
        Err(_) => return EditorConfigRules::default(),
    };

    let line_ending = props.get::<EndOfLine>().ok().map(|eol| match eol {
        EndOfLine::Lf | EndOfLine::Cr => LineEnding::Lf,
        EndOfLine::CrLf => LineEnding::Crlf,
    });

    let final_newline = props
        .get::<FinalNewline>()
        .ok()
        .map(|FinalNewline::Value(b)| b);

    let trim_trailing_whitespace = props
        .get::<TrimTrailingWs>()
        .ok()
        .map(|TrimTrailingWs::Value(b)| b);

    EditorConfigRules {
        line_ending,
        final_newline,
        trim_trailing_whitespace,
    }
}
