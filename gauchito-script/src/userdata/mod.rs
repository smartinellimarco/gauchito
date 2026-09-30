//! Userdata types crossing the Rust/Lua bridge — one module per concept.
//!
//! - [`LuaRope`]      opaque rope handle (`io.read` result → `buf.from_rope`).
//! - [`LuaBuffer`]    the editable buffer: editing, queries, motion kernels.
//!   Mutations borrow through `buffer::borrow_mut_buf`, so a
//!   nested edit surfaces as a clean Lua error rather than a `RefCell` panic.
//! - [`LuaView`]      a cursor session `{ id, buf, selection }` — pure substrate.
//! - [`LuaSelection`] an immutable selection snapshot.
//! - [`LuaSplice`]    the substrate edit primitive `(p, q, text)`.
//! - [`LuaFrame`]     the persistent paint buffer.

mod buffer;
mod frame;
mod rope;
mod selection;
mod splice;
mod view;

pub use buffer::LuaBuffer;
pub use frame::LuaFrame;
pub use rope::LuaRope;
pub use selection::LuaSelection;
pub use splice::LuaSplice;
pub use view::LuaView;
