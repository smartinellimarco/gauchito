//! `Frame` — the per-tick paint buffer.
//!
//! Three painting primitives + one piece of state:
//!
//! - [`Frame::text`]   — single-row run of styled spans.
//! - [`Frame::fill`]   — repeat a char in a rect with one style.
//! - [`Frame::box_`]   — draw a border around a rect (charset + style).
//! - [`Frame::cursor`] / [`Frame::set_cursor`] — terminal cursor state.
//!
//! The distro accumulates ops between `clear()` calls; the CLI flushes
//! each tick. `clear()` resets both the op list and the cursor.
//!
//! There is no `paint_highlight` primitive: the Lua-side painter
//! resolves styles per cell into a run sequence, and a single `text`
//! op renders the line.
//! One primitive does one job.

use ratatui::Frame as RatatuiFrame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::render;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub style: CursorStyle,
}

/// Border characters for [`Frame::box_`]. Use one of the named presets
/// (`Rounded`, `Double`, `Ascii`) or `Custom` for a bespoke charset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxCharset {
    Rounded,
    Double,
    Ascii,
    Custom(BoxChars),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxChars {
    pub tl: char,
    pub tr: char,
    pub bl: char,
    pub br: char,
    pub h: char,
    pub v: char,
}

impl BoxCharset {
    pub fn chars(self) -> BoxChars {
        match self {
            BoxCharset::Rounded => BoxChars {
                tl: '╭',
                tr: '╮',
                bl: '╰',
                br: '╯',
                h: '─',
                v: '│',
            },
            BoxCharset::Double => BoxChars {
                tl: '╔',
                tr: '╗',
                bl: '╚',
                br: '╝',
                h: '═',
                v: '║',
            },
            BoxCharset::Ascii => BoxChars {
                tl: '+',
                tr: '+',
                bl: '+',
                br: '+',
                h: '-',
                v: '|',
            },
            BoxCharset::Custom(c) => c,
        }
    }
}

/// A single styled run inside a [`PaintOp::Text`] op.
#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

pub enum PaintOp {
    /// Render `runs` left-to-right within `area`. Must be a single row
    /// (`area.h == 1`) — multi-row text is the caller's responsibility
    /// (split it into multiple ops). Total run width should match
    /// `area.w`; over-wide runs clip, under-wide leave trailing cells
    /// untouched.
    Text { area: Rect, runs: Vec<Run> },
    /// Fill every cell of `area` with `ch`, styled with `style`.
    Fill { area: Rect, ch: char, style: Style },
    /// Draw a single-cell border around `area` using `charset`, styled
    /// with `style`. Inside cells are untouched.
    Box {
        area: Rect,
        charset: BoxCharset,
        style: Style,
    },
}

pub struct Frame {
    ops: Vec<PaintOp>,
    cursor: Option<Cursor>,
    area: Rect,
}

impl Frame {
    pub fn new() -> Self {
        Frame {
            ops: Vec::new(),
            cursor: None,
            area: Rect::new(0, 0, 0, 0),
        }
    }

    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    /// Reset the per-tick paint state — clears both the op list and
    /// the cursor directive.
    pub fn clear(&mut self) {
        self.ops.clear();
        self.cursor = None;
    }

    /// Paint a single-row run of styled spans.
    pub fn text(&mut self, area: Rect, runs: Vec<Run>) {
        debug_assert!(
            area.height <= 1,
            "Frame::text expects a single-row area (h <= 1); got h={}",
            area.height
        );
        self.ops.push(PaintOp::Text { area, runs });
    }

    /// Fill a rect with a repeated char.
    pub fn fill(&mut self, area: Rect, ch: char, style: Style) {
        self.ops.push(PaintOp::Fill { area, ch, style });
    }

    /// Draw a border around `area`. Inner area is untouched — fill it
    /// separately if you want a background.
    pub fn box_(&mut self, area: Rect, charset: BoxCharset, style: Style) {
        self.ops.push(PaintOp::Box {
            area,
            charset,
            style,
        });
    }

    /// Set the terminal cursor for this tick. Repeated calls overwrite;
    /// position in the call stream is irrelevant — cursor is state.
    pub fn set_cursor(&mut self, x: u16, y: u16, style: CursorStyle) {
        self.cursor = Some(Cursor { x, y, style });
    }

    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    /// Walk the buffered ops and submit them to the terminal frame;
    /// apply the cursor directive (if any) afterward.
    pub fn flush(&self, terminal: &mut RatatuiFrame) {
        for op in &self.ops {
            match op {
                PaintOp::Text { area, runs } => {
                    render::paint_text(terminal, *area, runs);
                }
                PaintOp::Fill { area, ch, style } => {
                    render::paint_fill(terminal, *area, *ch, *style);
                }
                PaintOp::Box {
                    area,
                    charset,
                    style,
                } => {
                    render::paint_box(terminal, *area, charset.chars(), *style);
                }
            }
        }

        if let Some(c) = self.cursor {
            terminal.set_cursor_position(ratatui::layout::Position::new(c.x, c.y));
            apply_cursor_style(c.style);
        }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: more styles
fn apply_cursor_style(style: CursorStyle) {
    use crossterm::cursor::SetCursorStyle;
    let cs = match style {
        CursorStyle::Block => SetCursorStyle::SteadyBlock,
        CursorStyle::Bar => SetCursorStyle::SteadyBar,
    };
    let _ = crossterm::execute!(std::io::stdout(), cs);
}
