//! `gauchito.{selection, splice, view}` constructor namespaces.
//!
//! Pure value constructors — nothing here owns state. State-mutating
//! methods live on the userdata themselves (see `userdata.rs`).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::prelude::*;

use gauchito_core::selection::SelectionSnapshot;
use gauchito_core::splice::Splice;
use gauchito_core::View;

use crate::userdata::{LuaBuffer, LuaSelection, LuaSplice, LuaView};

// ── gauchito.splice ───────────────────────────────────────────────────────

pub fn register_splice(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set(
        "new",
        lua.create_function(|_, (p, q, text): (usize, usize, String)| {
            if p > q {
                return Err(LuaError::runtime(format!(
                    "splice.new: p ({p}) must be <= q ({q})"
                )));
            }
            Ok(LuaSplice(Splice::new(p, q, text)))
        })?,
    )?;
    Ok(t)
}

// ── gauchito.selection ────────────────────────────────────────────────────

pub fn register_selection(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;

    t.set(
        "new",
        lua.create_function(|_, ranges: LuaTable| {
            let mut out: Vec<(usize, usize)> = Vec::new();
            for v in ranges.sequence_values::<LuaTable>() {
                let r = v?;
                let a: usize = r.get("anchor")?;
                let h: usize = r.get("head")?;
                out.push((a, h));
            }
            if out.is_empty() {
                return Err(LuaError::runtime(
                    "selection.new: at least one range required",
                ));
            }
            Ok(LuaSelection(SelectionSnapshot {
                ranges: out,
                primary: 0,
            }))
        })?,
    )?;

    t.set(
        "point",
        lua.create_function(|_, pos: usize| {
            Ok(LuaSelection(SelectionSnapshot {
                ranges: vec![(pos, pos)],
                primary: 0,
            }))
        })?,
    )?;

    t.set(
        "range",
        lua.create_function(|lua, (a, h): (usize, usize)| {
            let r = lua.create_table()?;
            r.set("anchor", a)?;
            r.set("head", h)?;
            Ok(r)
        })?,
    )?;

    Ok(t)
}

// ── gauchito.view ─────────────────────────────────────────────────────────

pub fn register_view(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;

    // view.new(buf) — mount a fresh view on a buffer with a point
    // selection at 0. Selection pins release when the LuaView's last
    // Rc drops.
    t.set(
        "new",
        lua.create_function(|_, buf: LuaBuffer| {
            let view = View::new(buf.buf);
            Ok(LuaView {
                view: Rc::new(RefCell::new(view)),
            })
        })?,
    )?;

    Ok(t)
}
