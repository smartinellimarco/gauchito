//! Internal rendering helpers — execute paint ops against the ratatui
//! frame. Not exposed; the public surface is the `Frame` paint methods.

use ratatui::Frame as RatatuiFrame;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::frame::{BoxChars, Run};

/// Render a single-row run of styled spans into `area`. Caller is
/// responsible for matching total run width to `area.w`; over-wide
/// runs clip, under-wide leave trailing cells untouched.
pub(crate) fn paint_text(frame: &mut RatatuiFrame, area: Rect, runs: &[Run]) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let spans: Vec<Span> = runs
        .iter()
        .map(|r| Span::styled(r.text.clone(), r.style))
        .collect();
    let row = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(Line::from(spans), row);
}

/// Fill every cell of `area` with `ch`, styled with `style`. Writes
/// cells directly — no per-row String allocation.
pub(crate) fn paint_fill(frame: &mut RatatuiFrame, area: Rect, ch: char, style: Style) {
    let buf = frame.buffer_mut();
    let clipped = area.intersection(buf.area);
    for y in clipped.top()..clipped.bottom() {
        for x in clipped.left()..clipped.right() {
            let cell: &mut Cell = &mut buf[(x, y)];
            cell.set_char(ch);
            cell.set_style(style);
        }
    }
}

/// Draw a 1-cell border around `area` using `chars`. Degenerate sizes
/// (w<2 or h<2) are no-ops; corners draw on top of edges as expected.
pub(crate) fn paint_box(frame: &mut RatatuiFrame, area: Rect, chars: BoxChars, style: Style) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let buf = frame.buffer_mut();
    let clipped = area.intersection(buf.area);
    if clipped.width < 2 || clipped.height < 2 {
        return;
    }

    let left = clipped.left();
    let right = clipped.right() - 1;
    let top = clipped.top();
    let bottom = clipped.bottom() - 1;

    // Top + bottom edges.
    for x in left + 1..right {
        let c = &mut buf[(x, top)];
        c.set_char(chars.h);
        c.set_style(style);
        let c = &mut buf[(x, bottom)];
        c.set_char(chars.h);
        c.set_style(style);
    }
    // Left + right edges.
    for y in top + 1..bottom {
        let c = &mut buf[(left, y)];
        c.set_char(chars.v);
        c.set_style(style);
        let c = &mut buf[(right, y)];
        c.set_char(chars.v);
        c.set_style(style);
    }
    // Corners.
    let c = &mut buf[(left, top)];
    c.set_char(chars.tl);
    c.set_style(style);
    let c = &mut buf[(right, top)];
    c.set_char(chars.tr);
    c.set_style(style);
    let c = &mut buf[(left, bottom)];
    c.set_char(chars.bl);
    c.set_style(style);
    let c = &mut buf[(right, bottom)];
    c.set_char(chars.br);
    c.set_style(style);
}
