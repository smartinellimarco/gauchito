//! Buffer ↔ disk — one-pass read with sniffing, options-honoring write.
//!
//! [`read`] walks the file line by line so we can sniff BOM and
//! CR/LF line endings as we build the rope — one pass, no rewind.
//! Sniffed values come back as a [`PartialBufferOptions`] overlay,
//! so the caller can layer distro defaults on top (see
//! [`crate::options`]).
//!
//! [`write`] reverses the conversions: rebuild LF lines into the
//! buffer's chosen line ending, optionally strip trailing
//! whitespace, optionally enforce a final newline. Clears `modified`
//! on success.

use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use ropey::{Rope, RopeBuilder};

use crate::buffer::{Buffer, LineEnding};
use crate::movement::LINES;
use crate::options::PartialBufferOptions;

pub fn read(path: &Path) -> io::Result<(Rope, PartialBufferOptions)> {
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

    let sniffed = PartialBufferOptions {
        line_ending:              saw_any_line.then_some(line_ending),
        final_newline:            saw_any_line.then_some(final_newline),
        bom:                      Some(bom),
        trim_trailing_whitespace: None,
        indent:                   None,
    };

    Ok((builder.finish(), sniffed))
}

pub fn write(buf: &mut Buffer) -> io::Result<()> {
    let path = buf
        .path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "buffer has no file path"))?;

    let file = std::fs::File::create(path)?;
    let mut w = io::BufWriter::new(file);

    if buf.options.bom {
        w.write_all("\u{feff}".as_bytes())?;
    }

    let sep = match buf.options.line_ending {
        LineEnding::Lf => "\n",
        LineEnding::Crlf => "\r\n",
    };

    for line in buf.text.lines(LINES) {
        let mut content: String = line.chars().collect();

        let had_newline = content.ends_with('\n');
        if had_newline {
            content.pop();
        }

        if buf.options.trim_trailing_whitespace {
            let trimmed = content.trim_end();
            content.truncate(trimmed.len());
        }

        w.write_all(content.as_bytes())?;

        if had_newline {
            w.write_all(sep.as_bytes())?;
        }
    }

    if buf.options.final_newline {
        let len = buf.text.len();
        let ends_with_nl = len > 0 && buf.text.byte(len - 1) == b'\n';
        if !ends_with_nl {
            w.write_all(sep.as_bytes())?;
        }
    }

    w.flush()?;
    buf.modified = false;
    Ok(())
}
