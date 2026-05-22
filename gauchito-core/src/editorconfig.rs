use std::path::Path;

use ec4rs::property::{
    EndOfLine, FinalNewline, IndentSize, IndentStyle as EcIndentStyle, TabWidth, TrimTrailingWs,
};

use crate::document::{IndentStyle, LineEnding};
use crate::options::PartialDocumentOptions;

pub fn rules_for(path: &Path) -> PartialDocumentOptions {
    let Ok(props) = ec4rs::properties_of(path) else {
        return PartialDocumentOptions::default();
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

    let style = props.get::<EcIndentStyle>().ok();
    let size = props.get::<IndentSize>().ok();
    let tab_width_prop = props.get::<TabWidth>().ok().map(|TabWidth::Value(n)| n);

    let indent = match (style, size) {
        (Some(EcIndentStyle::Tabs), _) => Some(IndentStyle {
            unit: "\t".to_string(),
            tab_width: tab_width_prop.unwrap_or(8) as u8,
        }),
        (Some(EcIndentStyle::Spaces), Some(IndentSize::Value(n))) => Some(IndentStyle {
            unit: " ".repeat(n),
            tab_width: tab_width_prop.unwrap_or(n) as u8,
        }),
        (Some(EcIndentStyle::Spaces), _) => Some(IndentStyle {
            unit: "    ".to_string(),
            tab_width: tab_width_prop.unwrap_or(4) as u8,
        }),
        (None, Some(IndentSize::Value(n))) => Some(IndentStyle {
            unit: " ".repeat(n),
            tab_width: tab_width_prop.unwrap_or(n) as u8,
        }),
        (None, _) => tab_width_prop.map(|w| IndentStyle {
            unit: "    ".to_string(),
            tab_width: w as u8,
        }),
    };

    PartialDocumentOptions {
        line_ending,
        final_newline,
        bom: None,
        trim_trailing_whitespace,
        indent,
    }
}
