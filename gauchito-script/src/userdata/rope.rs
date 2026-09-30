//! Rope userdata — opaque rope handle.
//!
//! The seam between `gauchito.io.read` and `gauchito.buf.from_rope`.
//! Exposes only the queries a preview path needs (`len`, `line`) —
//! anything richer should go through the buffer userdata. Cheap to
//! clone (ropey's B-tree is Arc-shared internally).

use mlua::prelude::*;
use ropey::Rope;

use gauchito_core::movement::LINES;

/// Opaque rope handle. Used only as the intermediate result of
/// `gauchito.io.read(path)` so distros can pass it to
/// `gauchito.buf.from_rope(rope, path, opts)`. Cheap to clone (ropey's
/// B-tree is Arc-shared internally).
#[derive(Clone)]
pub struct LuaRope(pub Rope);

impl FromLua for LuaRope {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected Rope")),
        }
    }
}

impl LuaUserData for LuaRope {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| Ok(this.0.len()));
        methods.add_method("line", |_, this, n: usize| {
            if n >= this.0.len_lines(LINES) {
                return Ok(String::new());
            }
            let s: String = this.0.line(n, LINES).chars().take_while(|&c| c != '\n').collect();
            Ok(s.trim_end_matches('\r').to_string())
        });
    }
}
