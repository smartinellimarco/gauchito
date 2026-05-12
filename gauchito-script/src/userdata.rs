//! Plain-data userdata types passed between Lua and Rust.
//!
//! - [`LuaBuffer`] — a clone of a document's [`Rope`]. Cheap (Arc-shared).
//! - [`LuaSelection`] — a [`SelectionSnapshot`] (resolved offsets). Lua never
//!   sees [`AnchorId`](gauchito_core::anchor::AnchorId)s; the bridge rehydrates
//!   on `ctx:set_selection`.
//! - [`LuaChangeSet`] — opaque [`ChangeSet`] handle. Lua passes it back via
//!   `ctx:edit`.
//!
//! These are deliberately dumb wrappers. All algebra lives in `prelude.lua`
//! and user presets; all mutation goes through `Ctx` methods.

use mlua::prelude::*;
use ropey::Rope;

use gauchito_core::changeset::ChangeSet;
use gauchito_core::history::SelectionSnapshot;

// ── LuaBuffer ──────────────────────────────────────────────────────────────

/// Read-only view of a rope. Cloned cheaply (B-tree of `Arc` nodes).
#[derive(Clone)]
pub struct LuaBuffer(pub Rope);

impl FromLua for LuaBuffer {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected LuaBuffer")),
        }
    }
}

impl LuaUserData for LuaBuffer {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| Ok(this.0.len_chars()));

        methods.add_method("char", |_, this, pos: usize| {
            if pos >= this.0.len_chars() {
                return Ok(String::new());
            }
            Ok(this.0.char(pos).to_string())
        });

        methods.add_method("slice", |_, this, (from, to): (usize, usize)| {
            let to = to.min(this.0.len_chars());
            let from = from.min(to);
            Ok(this.0.slice(from..to).to_string())
        });

        methods.add_method("len_lines", |_, this, ()| Ok(this.0.len_lines()));

        methods.add_method("char_to_line", |_, this, pos: usize| {
            let len = this.0.len_chars();
            let pos = if len == 0 { 0 } else { pos.min(len - 1) };
            Ok(this.0.char_to_line(pos))
        });
    }
}

// ── LuaSelection ───────────────────────────────────────────────────────────

/// Selection as a list of (anchor_offset, head_offset) pairs. Lua-friendly,
/// no `AnchorId`s. The bridge rehydrates on `ctx:set_selection`.
#[derive(Clone)]
pub struct LuaSelection(pub SelectionSnapshot);

impl FromLua for LuaSelection {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected LuaSelection")),
        }
    }
}

impl LuaUserData for LuaSelection {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| Ok(this.0.ranges.len()));

        methods.add_method("primary_idx", |_, this, ()| Ok(this.0.primary));

        methods.add_method("primary", |lua, this, ()| {
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
    }
}

// ── LuaChangeSet ───────────────────────────────────────────────────────────

/// Opaque changeset handle. Lua doesn't introspect; it passes via `ctx:edit`.
#[derive(Clone)]
pub struct LuaChangeSet(pub ChangeSet);

impl FromLua for LuaChangeSet {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected LuaChangeSet")),
        }
    }
}

impl LuaUserData for LuaChangeSet {}
