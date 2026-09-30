//! Gauchito substrate — the runtime everything else composes on.
//!
//! The crate's spine is "everything that follows edits is a pin":
//! text lives in a [`Buffer`], edits are [`Splice`]s, and anything
//! that should ride those edits (selection ranges) is a [`PinId`] in
//! the buffer's [`PinTable`], projected by ϕ on every apply.
//!
//! The crate is intentionally Lua-agnostic — `gauchito-script` is
//! the only place mlua is allowed. Distro policy (modes, keymaps,
//! splits, scrolling) lives above the substrate, never inside it.

pub mod pins;
pub mod buffer;
pub mod grapheme;
pub mod io;
pub mod movement;
pub mod options;
pub mod paths;
pub mod selection;
pub mod splice;
pub mod view;

pub use pins::{PinId, PinTable};
pub use buffer::{Buffer, BufferId, BufferOptions, IndentStyle, LineEnding};
pub use selection::{Range, Selection};
pub use splice::{Gravity, Origin, Splice, phi};
pub use view::{View, ViewId};
