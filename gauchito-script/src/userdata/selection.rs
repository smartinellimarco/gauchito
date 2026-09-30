//! Selection userdata — immutable snapshot of a [`SelectionSnapshot`].
//!
//! Snapshots cross the Rust ↔ Lua boundary as plain offsets, so the
//! Lua side never has to think about pins. Mutators return a new
//! snapshot rather than mutating in place — keeps the Lua surface
//! equational and pushes live, pin-tracked mutation through the
//! typed core `Selection` via `view:set_selection`.

use mlua::prelude::*;

use gauchito_core::selection::SelectionSnapshot;

#[derive(Clone)]
pub struct LuaSelection(pub SelectionSnapshot);

impl FromLua for LuaSelection {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected Selection")),
        }
    }
}

impl LuaUserData for LuaSelection {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("ranges", |lua, this| {
            let t = lua.create_table()?;
            for (i, &(a, h)) in this.0.ranges.iter().enumerate() {
                let r = lua.create_table()?;
                r.set("anchor", a)?;
                r.set("head", h)?;
                t.set(i + 1, r)?;
            }
            Ok(t)
        });

        fields.add_field_method_get("primary", |_, this| Ok(this.0.primary));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| Ok(this.0.ranges.len()));

        methods.add_method("primary_idx", |_, this, ()| Ok(this.0.primary));

        methods.add_method("primary_range", |lua, this, ()| {
            let (anchor, head) = this.0.ranges[this.0.primary];
            let t = lua.create_table()?;
            t.set("anchor", anchor)?;
            t.set("head", head)?;
            Ok(t)
        });

        methods.add_method("range", |lua, this, i: usize| {
            if i >= this.0.ranges.len() {
                return Err(LuaError::runtime("range index out of bounds"));
            }
            let (anchor, head) = this.0.ranges[i];
            let t = lua.create_table()?;
            t.set("anchor", anchor)?;
            t.set("head", head)?;
            Ok(t)
        });

        methods.add_method("add_range", |_, this, (anchor, head): (usize, usize)| {
            let mut snap = this.0.clone();
            snap.ranges.push((anchor, head));
            snap.primary = snap.ranges.len() - 1;
            Ok(LuaSelection(snap))
        });

        methods.add_method("remove", |_, this, idx: usize| {
            if idx >= this.0.ranges.len() || this.0.ranges.len() == 1 {
                return Ok(LuaSelection(this.0.clone()));
            }
            let mut snap = this.0.clone();
            snap.ranges.remove(idx);
            if snap.primary >= snap.ranges.len() {
                snap.primary = snap.ranges.len() - 1;
            }
            Ok(LuaSelection(snap))
        });

        methods.add_method("set_primary", |_, this, i: usize| {
            let mut snap = this.0.clone();
            if i < snap.ranges.len() {
                snap.primary = i;
            }
            Ok(LuaSelection(snap))
        });
    }
}
