//! View userdata — Lua handle on a [`View`] `{ id, buf, selection }`.
//!
//! Selection mutators route through [`replace_view_selection`] so
//! the old selection's pins are released back to the buffer's pin
//! table before the new selection installs — pins are
//! buffer-scoped, and a leaked pin lives until the buffer dies.
//!
//! `set_buf` issues a fresh point selection on the new buffer
//! because pin ids don't translate across pin tables.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::prelude::*;

use gauchito_core::selection::SelectionSnapshot;
use gauchito_core::{Selection, View};

use super::buffer::LuaBuffer;
use super::selection::LuaSelection;

#[derive(Clone)]
pub struct LuaView {
    pub view: Rc<RefCell<View>>,
}

impl FromLua for LuaView {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected View")),
        }
    }
}

impl LuaUserData for LuaView {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.view.borrow().id.0));

        fields.add_field_method_get("buf", |_, this| {
            Ok(LuaBuffer {
                buf: this.view.borrow().buf.clone(),
            })
        });

        fields.add_field_method_get("selection", |_, this| {
            let view = this.view.borrow();
            let buf = view.buf.borrow();
            Ok(LuaSelection(view.selection.snapshot(&buf.pins)))
        });
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set_selection", |_, this, sel: LuaSelection| {
            replace_view_selection(&this.view, sel.0);
            Ok(())
        });

        methods.add_method("snapshot", |_, this, ()| {
            let view = this.view.borrow();
            let buf = view.buf.borrow();
            Ok(LuaSelection(view.selection.snapshot(&buf.pins)))
        });

        methods.add_method("restore", |_, this, snap: LuaSelection| {
            replace_view_selection(&this.view, snap.0);
            Ok(())
        });

        methods.add_method("set_buf", |_, this, buf: LuaBuffer| {
            let new_selection = Selection::point(&mut buf.buf.borrow_mut().pins, 0);

            let mut view = this.view.borrow_mut();
            let old_buf_rc = view.buf.clone();
            let mut old_selection = std::mem::replace(&mut view.selection, new_selection);
            view.buf = buf.buf.clone();
            drop(view);

            old_selection.drop(&mut old_buf_rc.borrow_mut().pins);
            Ok(())
        });
    }
}

fn replace_view_selection(view_rc: &Rc<RefCell<View>>, snap: SelectionSnapshot) {
    let buf_rc = view_rc.borrow().buf.clone();
    let mut buf = buf_rc.borrow_mut();
    let new_sel = Selection::from_snapshot(&mut buf.pins, &snap);
    let mut view = view_rc.borrow_mut();
    let mut old = std::mem::replace(&mut view.selection, new_sel);
    old.drop(&mut buf.pins);
}
