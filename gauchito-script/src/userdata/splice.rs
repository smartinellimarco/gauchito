//! Splice userdata — read-only `(p, q, text)` view.
//!
//! Splices are constructed via `gauchito.splice.new` and consumed
//! by `buf:apply`. Lua never edits a splice in place; mutating
//! buffer text is the buffer's job, not the splice's.

use mlua::prelude::*;

use gauchito_core::splice::Splice;

#[derive(Clone)]
pub struct LuaSplice(pub Splice);

impl FromLua for LuaSplice {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected Splice")),
        }
    }
}

impl LuaUserData for LuaSplice {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("p", |_, this| Ok(this.0.p()));
        fields.add_field_method_get("q", |_, this| Ok(this.0.q()));
        fields.add_field_method_get("text", |_, this| Ok(this.0.text().to_string()));
    }
}
