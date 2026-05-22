use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use ropey::{Rope, RopeBuilder};

use crate::document::{Document, DocumentOptions, LineEnding};
use crate::editorconfig;
use crate::options::PartialDocumentOptions;

pub fn read(path: &Path) -> io::Result<(Rope, PartialDocumentOptions)> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut builder = RopeBuilder::new();
    let mut buf = String::new();

    let mut first = true;
    let mut bom = false;
    let mut line_ending = LineEnding::Lf;
    let mut final_newline = false;
    let mut saw_any_line = false;

    loop {
        buf.clear();
        if reader.read_line(&mut buf)? == 0 {
            break;
        }
        saw_any_line = true;

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

    let sniffed = PartialDocumentOptions {
        line_ending: saw_any_line.then_some(line_ending),
        final_newline: saw_any_line.then_some(final_newline),
        bom: Some(bom),
        trim_trailing_whitespace: None,
        indent: None,
    };

    Ok((builder.finish(), sniffed))
}

pub fn load(path: PathBuf) -> io::Result<Document> {
    let path = std::fs::canonicalize(&path).unwrap_or(path);

    let (rope, options) = if path.exists() {
        let (rope, sniffed) = read(&path)?;
        let options = PartialDocumentOptions::default()
            .merge(editorconfig::rules_for(&path))
            .merge(sniffed)
            .resolve(DocumentOptions::default());
        (rope, options)
    } else {
        let options = editorconfig::rules_for(&path).resolve(DocumentOptions::default());
        (Rope::new(), options)
    };

    Ok(Document::new(rope, Some(path), options))
}

pub fn write(doc: &Document) -> io::Result<()> {
    let path = doc
        .path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "document has no file path"))?;

    let file = std::fs::File::create(path)?;
    let mut w = io::BufWriter::new(file);

    if doc.options.bom {
        w.write_all("\u{feff}".as_bytes())?;
    }

    let sep = {
        let this = doc.options.line_ending;
        match this {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        }
    };

    for line in doc.text.lines() {
        let mut content: String = line.chars().collect();

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

    if doc.options.final_newline {
        let len = doc.text.len_chars();
        let ends_with_nl = len > 0 && doc.text.char(len - 1) == '\n';
        if !ends_with_nl {
            w.write_all(sep.as_bytes())?;
        }
    }

    w.flush()
}
