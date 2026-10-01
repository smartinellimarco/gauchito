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

#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

pub enum PaintOp {
    Text {
        area: Rect,
        runs: Vec<Run>,
    },
    Fill {
        area: Rect,
        ch: char,
        style: Style,
    },
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

    pub fn clear(&mut self) {
        self.ops.clear();
        self.cursor = None;
    }

    pub fn text(&mut self, area: Rect, runs: Vec<Run>) {
        debug_assert!(
            area.height <= 1,
            "Frame::text expects a single-row area (h <= 1); got h={}",
            area.height
        );
        self.ops.push(PaintOp::Text { area, runs });
    }

    pub fn fill(&mut self, area: Rect, ch: char, style: Style) {
        self.ops.push(PaintOp::Fill { area, ch, style });
    }

    pub fn box_(&mut self, area: Rect, charset: BoxCharset, style: Style) {
        self.ops.push(PaintOp::Box {
            area,
            charset,
            style,
        });
    }

    pub fn set_cursor(&mut self, x: u16, y: u16, style: CursorStyle) {
        self.cursor = Some(Cursor { x, y, style });
    }

    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

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
