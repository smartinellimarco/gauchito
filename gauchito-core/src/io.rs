use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use ropey::{Rope, RopeBuilder};

use crate::document::Document;
use crate::editorconfig;
use crate::options::{DocumentOptions, LineEnding};

pub fn load(path: PathBuf) -> io::Result<Document> {
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    let rules = editorconfig::rules_for(&path);

    let (text, options) = if path.exists() {
        let (rope, line_ending, final_newline, bom) = read_and_sniff(&path)?;
        let options = DocumentOptions::resolve(
            line_ending,
            final_newline,
            bom,
            rules.line_ending,
            rules.final_newline,
            rules.trim_trailing_whitespace,
        );
        (rope, options)
    } else {
        let options = DocumentOptions::resolve(
            LineEnding::Lf,
            true,
            false,
            rules.line_ending,
            rules.final_newline,
            rules.trim_trailing_whitespace,
        );
        (Rope::new(), options)
    };

    let mut doc = Document::from_rope(text);
    doc.path = Some(path);
    doc.options = options;
    Ok(doc)
}

/// Read a file line-by-line, sniffing BOM, line-ending style, and final newline.
/// Returns the rope (with `\r\n` normalized to `\n`) and the sniffed properties.
fn read_and_sniff(path: &Path) -> io::Result<(Rope, LineEnding, bool, bool)> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut builder = RopeBuilder::new();
    let mut buf = String::new();

    let mut first = true;
    let mut bom = false;
    let mut line_ending = LineEnding::Lf;
    let mut final_newline = false;

    loop {
        buf.clear();
        if reader.read_line(&mut buf)? == 0 {
            break;
        }

        if first {
            first = false;

            if buf.starts_with('\u{feff}') {
                bom = true;
                buf.drain(..'\u{feff}'.len_utf8());
            }

            if buf.ends_with("\r\n") {
                line_ending = LineEnding::Crlf;
            }
        }

        final_newline = buf.ends_with('\n');

        if buf.ends_with("\r\n") {
            buf.truncate(buf.len() - 2);
            buf.push('\n');
        }

        builder.append(&buf);
    }

    Ok((builder.finish(), line_ending, final_newline, bom))
}

pub fn write(doc: &Document) -> io::Result<()> {
    let path = doc
        .path
        .as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "document has no file path"))?;

    let file = std::fs::File::create(path)?;
    let mut w = io::BufWriter::new(file);

    if doc.options.bom {
        w.write_all("\u{feff}".as_bytes())?;
    }

    let sep = doc.options.line_ending.as_str();

    for line in doc.text.lines() {
        let mut content: String = line.chars().collect();

        // Strip the trailing \n that ropey keeps on every line except possibly the last.
        let had_newline = content.ends_with('\n');
        if had_newline {
            content.pop();
        }

        if doc.options.trim_trailing_whitespace {
            let trimmed = content.trim_end();
            content.truncate(trimmed.len());
        }

        w.write_all(content.as_bytes())?;

        if had_newline {
            w.write_all(sep.as_bytes())?;
        }
    }

    // Final newline policy.
    if doc.options.final_newline {
        let len = doc.text.len_chars();
        let ends_with_nl = len > 0 && doc.text.char(len - 1) == '\n';
        if !ends_with_nl {
            w.write_all(sep.as_bytes())?;
        }
    }

    w.flush()
}
