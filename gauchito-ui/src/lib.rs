//! Gauchito UI — paint primitives the distro composes into an editor.
//!
//! One type: [`Frame`]. The distro paints into it via three
//! primitives — `text` (runs of styled spans), `fill` (rect of one
//! char), `box_` (single-cell border) — plus a cursor state slot.
//! Ops accumulate until the CLI calls [`Frame::flush`], which walks
//! them and writes to the terminal.
//!
//! There is no `paint_highlight` primitive, on purpose. The original
//! four-primitive design (text + fill + highlight + cursor) was
//! dishonest: `paint_text` was monochromatic, so `paint_highlight`
//! did the real work via cell-level patches — ~1300 ops/frame, half
//! of them cell-walking. Admitting text is already styled — runs of
//! `(text, style)` — let one primitive do one job (2026-05-26 RFC
//! review).
//!
//! Concept-aware painting (paint a view, paint a selection, paint a
//! popup) lives in Lua (`gauchito.paint.*`), built from these three.

pub mod frame;
mod render;

pub use frame::{Cursor, CursorStyle, Frame, PaintOp};
pub use ratatui::layout::Rect;
pub use ratatui::style::{Color, Modifier, Style};
pub use ratatui::text::Span;
