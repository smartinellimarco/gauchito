//! Buffer userdata — the editable thing on the Lua side.
//!
//! Mirrors the substrate [`gauchito_core::Buffer`] plus the motion
//! kernels. Mutating ops route through
//! [`borrow_mut_buf`], which turns a `RefCell` double-borrow (nested
//! edit, two handles mutating the same buffer) into a clean Lua
//! error naming the buffer. The alternative is a Rust panic
//! surfaced through mlua as an unrecoverable runtime fault — a hard
//! crash where a recoverable script error belongs.

use std::cell::{RefCell, RefMut};
use std::rc::Rc;

use mlua::prelude::*;

use gauchito_core::movement::{
    CharClass, char_class, last_navigable_line, move_doc_end, move_first_non_whitespace,
    move_left, move_left_inline, move_line_end, move_line_start, move_right, move_right_inline,
    move_to_line, move_vertical, position_at, skip_class_backward, skip_class_forward,
    visual_column, LINES,
};
use gauchito_core::Buffer;

use super::splice::LuaSplice;

/// Handle to a buffer — the editable, persistent thing. Holds editing
/// methods, rope queries, and motion kernels.
#[derive(Clone)]
pub struct LuaBuffer {
    pub buf: Rc<RefCell<Buffer>>,
}

impl FromLua for LuaBuffer {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => Err(LuaError::runtime("expected Buffer")),
        }
    }
}

impl LuaUserData for LuaBuffer {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.buf.borrow().id.0));
        fields.add_field_method_get("path", |_, this| {
            Ok(this
                .buf
                .borrow()
                .path()
                .and_then(|p| p.to_str().map(String::from)))
        });
        fields.add_field_method_get("revision", |_, this| Ok(this.buf.borrow().revision));
        fields.add_field_method_get("name", |_, this| Ok(this.buf.borrow().name().to_string()));
        // `modified` is auto-managed: `buf:apply` sets it true; `buf:save`
        // sets it false. No setter is exposed on purpose — there's no
        // good reason for a distro to forge this state.
        fields.add_field_method_get("modified", |_, this| Ok(this.buf.borrow().modified));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // ── Editing ──────────────────────────────────────────────────────

        methods.add_method("apply", |_, this, splices: Vec<LuaSplice>| {
            let mut buf = borrow_mut_buf(this, "apply")?;
            let mut inverses = Vec::with_capacity(splices.len());
            for s in splices {
                boundary(&buf, s.0.p())?;
                boundary(&buf, s.0.q())?;
                inverses.push(LuaSplice(buf.apply(&s.0)));
            }
            Ok(inverses)
        });

        methods.add_method("save", |_, this, ()| {
            let mut buf = borrow_mut_buf(this, "save")?;
            gauchito_core::io::write(&mut buf)
                .map_err(|e| LuaError::runtime(format!("buf:save: {e}")))?;
            Ok(())
        });

        // ── Queries ──────────────────────────────────────────────────────

        methods.add_method("len", |_, this, ()| Ok(this.buf.borrow().text.len()));

        methods.add_method("line_count", |_, this, ()| {
            let buf = this.buf.borrow();
            Ok(last_navigable_line(&buf.text.slice(..)) + 1)
        });

        methods.add_method("char", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            if pos >= buf.text.len() {
                return Ok(String::new());
            }
            Ok(buf.text.char(boundary(&buf, pos)?).to_string())
        });

        methods.add_method("slice", |_, this, (from, to): (usize, usize)| {
            let buf = this.buf.borrow();
            let to = boundary(&buf, to.min(buf.text.len()))?;
            let from = boundary(&buf, from.min(to))?;
            Ok(buf.text.slice(from..to).to_string())
        });

        methods.add_method("line", |_, this, n: usize| {
            let buf = this.buf.borrow();
            if n >= buf.text.len_lines(LINES) {
                return Ok(String::new());
            }
            let s: String = buf.text.line(n, LINES).chars().take_while(|&c| c != '\n').collect();
            Ok(s.trim_end_matches('\r').to_string())
        });

        methods.add_method("bol", |_, this, n: usize| {
            let buf = this.buf.borrow();
            Ok(move_to_line(&buf.text.slice(..), n))
        });

        methods.add_method("eol", |_, this, n: usize| {
            let buf = this.buf.borrow();
            let text = buf.text.slice(..);
            Ok(move_line_end(&text, move_to_line(&text, n)))
        });

        methods.add_method("line_at", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            let len = buf.text.len();
            if len == 0 {
                return Ok(0);
            }
            // pos at or past EOF lands on the trailing empty line when
            // the file ends with `\n`. Without this carve-out, line_at
            // would map EOF back onto the previous line via
            // char_to_line clamping, hiding the empty last row.
            if pos >= len {
                let last = buf.text.byte(len - 1);
                if last == b'\n' || last == b'\r' {
                    return Ok(buf.text.len_lines(LINES) - 1);
                }
                return Ok(buf.text.byte_to_line_idx(len - 1, LINES));
            }
            Ok(buf.text.byte_to_line_idx(boundary(&buf, pos)?, LINES))
        });

        methods.add_method("char_class", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            if pos >= buf.text.len() {
                return Ok("eol");
            }
            Ok(class_tag(char_class(buf.text.char(boundary(&buf, pos)?))))
        });

        methods.add_method("scan_class_fwd", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(skip_class_forward(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("scan_class_bwd", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(skip_class_backward(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        // ── Motion kernels (position → position) ─────────────────────────

        methods.add_method("left", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_left(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("right", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_right(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("left_inline", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_left_inline(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("right_inline", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_right_inline(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("up", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_vertical(&buf.text.slice(..), boundary(&buf, pos)?, -1, None))
        });

        methods.add_method("down", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_vertical(&buf.text.slice(..), boundary(&buf, pos)?, 1, None))
        });

        methods.add_method("line_start_of", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_line_start(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("line_end_of", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_line_end(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("first_non_ws", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(move_first_non_whitespace(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("doc_end", |_, this, ()| {
            let buf = this.buf.borrow();
            Ok(move_doc_end(&buf.text.slice(..), 0))
        });

        methods.add_method("visual_col", |_, this, pos: usize| {
            let buf = this.buf.borrow();
            Ok(visual_column(&buf.text.slice(..), boundary(&buf, pos)?))
        });

        methods.add_method("pos_at", |_, this, (line, col): (usize, usize)| {
            let buf = this.buf.borrow();
            Ok(position_at(&buf.text.slice(..), line, col))
        });

    }
}

fn boundary(buf: &Buffer, pos: usize) -> LuaResult<usize> {
    if pos <= buf.text.len() && buf.text.is_char_boundary(pos) {
        Ok(pos)
    } else {
        Err(LuaError::runtime(format!("position {pos} is not a char boundary")))
    }
}

fn class_tag(cls: CharClass) -> &'static str {
    match cls {
        CharClass::Eol => "eol",
        CharClass::Whitespace => "whitespace",
        CharClass::Word => "word",
        CharClass::Punct => "punct",
    }
}

/// Borrow the buffer mutably, converting a `RefCell` double-borrow — which
/// would otherwise panic across the Lua boundary — into a catchable Lua
/// error that names the buffer and the likely cause. The realistic trigger
/// is a nested edit: mutating a buffer while another borrow on the *same*
/// buffer is still live (editing from a callback that fired mid-edit, or two
/// handles to one buffer mutating at once).
fn borrow_mut_buf<'a>(this: &'a LuaBuffer, op: &str) -> LuaResult<RefMut<'a, Buffer>> {
    this.buf.try_borrow_mut().map_err(|_| {
        let which = match this.buf.try_borrow() {
            Ok(b) => format!(" #{}", b.id.0),
            Err(_) => String::new(),
        };
        LuaError::runtime(format!(
            "buf:{op}: buffer{which} is already in use — a nested edit while \
             another borrow on the same buffer is still live"
        ))
    })
}
