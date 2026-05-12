//! Pure motion + mutation kernels exposed to Lua under the global `bv` table.
//!
//! All kernels operate on [`LuaBuffer`] / [`LuaSelection`] — they never touch
//! `EditorState`. They're plain functions: same input, same output. The preset
//! composes them via combinators in `prelude.lua` (`bv.collapse`, `bv.fold`,
//! …) into ctx-actions that the bridge then applies.
//!
//! Kernel shapes:
//! - Motion:     `bv.k.*(buf, head)         -> head`
//! - Selection:  `bv.k.*(buf, anchor, head) -> {anchor, head}`
//! - Char find:  `bv.k.*(buf, head, ch)     -> head`
//! - Mutation:   `bv.*(buf, sel)            -> changeset`

use mlua::prelude::*;

use gauchito_core::{edits, movement};
use gauchito_core::history::SelectionSnapshot;

use crate::userdata::{LuaBuffer, LuaChangeSet, LuaSelection};

pub fn register(lua: &Lua) -> LuaResult<()> {
    let bv = lua.create_table()?;
    let k = lua.create_table()?;

    register_motion_kernels(lua, &k)?;
    register_selection_kernels(lua, &k)?;
    register_char_kernels(lua, &k)?;

    bv.set("k", k)?;

    register_mutations(lua, &bv)?;

    lua.globals().set("bv", bv)?;
    Ok(())
}

// ── Motions: (buf, head) -> head ────────────────────────────────────────────

fn register_motion_kernels(lua: &Lua, k: &LuaTable) -> LuaResult<()> {
    macro_rules! kernel {
        ($name:ident) => {
            k.set(
                stringify!($name),
                lua.create_function(|_, (buf, head): (LuaBuffer, usize)| {
                    Ok(movement::$name(&buf.0.slice(..), head))
                })?,
            )?;
        };
    }

    kernel!(move_left);
    kernel!(move_right);
    kernel!(move_left_inline);
    kernel!(move_right_inline);
    kernel!(move_up);
    kernel!(move_down);
    kernel!(move_word_forward);
    kernel!(move_word_backward);
    kernel!(move_word_end);
    kernel!(move_word_forward_big);
    kernel!(move_word_backward_big);
    kernel!(move_word_end_big);
    kernel!(move_line_start);
    kernel!(move_line_end);
    kernel!(move_first_non_whitespace);
    kernel!(move_doc_start);
    kernel!(move_doc_end);
    kernel!(move_paragraph_forward);
    kernel!(move_paragraph_backward);
    kernel!(match_bracket);

    Ok(())
}

// ── Selection-shape kernels: (buf, anchor, head) -> (anchor, head) ─────────

fn register_selection_kernels(lua: &Lua, k: &LuaTable) -> LuaResult<()> {
    macro_rules! kernel {
        ($name:ident) => {
            k.set(
                stringify!($name),
                lua.create_function(|lua, (buf, anchor, head): (LuaBuffer, usize, usize)| {
                    let (a, h) = movement::$name(&buf.0.slice(..), anchor, head);
                    let t = lua.create_table()?;
                    t.set("anchor", a)?;
                    t.set("head", h)?;
                    Ok(t)
                })?,
            )?;
        };
    }

    kernel!(ensure_char_selected);
    kernel!(clamp_visible);
    kernel!(head_to_start);
    kernel!(head_to_end);
    kernel!(select_whole_line);

    Ok(())
}

// ── Char-find: (buf, head, ch) -> head ─────────────────────────────────────

fn register_char_kernels(lua: &Lua, k: &LuaTable) -> LuaResult<()> {
    macro_rules! kernel {
        ($name:ident) => {
            k.set(
                stringify!($name),
                lua.create_function(|_, (buf, head, ch): (LuaBuffer, usize, String)| {
                    let c = ch.chars().next().ok_or_else(|| LuaError::runtime("empty char"))?;
                    Ok(movement::$name(&buf.0.slice(..), head, c))
                })?,
            )?;
        };
    }

    kernel!(find_char_forward);
    kernel!(find_char_backward);
    kernel!(find_char_forward_before);
    kernel!(find_char_backward_after);

    Ok(())
}

// ── Mutations: (buf, sel) -> changeset ─────────────────────────────────────

fn register_mutations(lua: &Lua, bv: &LuaTable) -> LuaResult<()> {
    fn ranges(snap: &SelectionSnapshot) -> Vec<(usize, usize)> {
        snap.ranges
            .iter()
            .map(|&(a, h)| (a.min(h), a.max(h)))
            .collect()
    }

    fn heads(snap: &SelectionSnapshot) -> Vec<usize> {
        snap.ranges.iter().map(|&(_, h)| h).collect()
    }

    bv.set(
        "delete_selection",
        lua.create_function(|_, (buf, sel): (LuaBuffer, LuaSelection)| {
            Ok(LuaChangeSet(edits::delete_selection(
                &buf.0.slice(..),
                &ranges(&sel.0),
            )))
        })?,
    )?;

    bv.set(
        "delete_char_backward",
        lua.create_function(|_, (buf, sel): (LuaBuffer, LuaSelection)| {
            Ok(LuaChangeSet(edits::delete_char_backward(
                &buf.0.slice(..),
                &ranges(&sel.0),
            )))
        })?,
    )?;

    bv.set(
        "delete_char_forward",
        lua.create_function(|_, (buf, sel): (LuaBuffer, LuaSelection)| {
            Ok(LuaChangeSet(edits::delete_char_forward(
                &buf.0.slice(..),
                &ranges(&sel.0),
            )))
        })?,
    )?;

    bv.set(
        "insert_char",
        lua.create_function(|_, (buf, sel, ch): (LuaBuffer, LuaSelection, String)| {
            let c = ch.chars().next().ok_or_else(|| LuaError::runtime("empty char"))?;
            Ok(LuaChangeSet(edits::insert_char(
                &buf.0.slice(..),
                &heads(&sel.0),
                c,
            )))
        })?,
    )?;

    bv.set(
        "insert_newline",
        lua.create_function(|_, (buf, sel): (LuaBuffer, LuaSelection)| {
            Ok(LuaChangeSet(edits::insert_char(
                &buf.0.slice(..),
                &heads(&sel.0),
                '\n',
            )))
        })?,
    )?;

    bv.set(
        "insert_tab",
        lua.create_function(|_, (buf, sel): (LuaBuffer, LuaSelection)| {
            Ok(LuaChangeSet(edits::insert_char(
                &buf.0.slice(..),
                &heads(&sel.0),
                '\t',
            )))
        })?,
    )?;

    // Insert an arbitrary string at every head. `edits::insert_char` only
    // takes a `char`, so we build the changeset directly here.
    bv.set(
        "insert_text",
        lua.create_function(
            |_, (buf, sel, text): (LuaBuffer, LuaSelection, String)| {
                use gauchito_core::changeset::ChangeBuilder;

                let doc_len = buf.0.len_chars();
                let mut positions: Vec<usize> = heads(&sel.0);
                positions.sort();
                positions.dedup();

                let mut b = ChangeBuilder::new(doc_len);
                for pos in positions {
                    b.advance_to(pos);
                    b.insert(&text);
                }
                Ok(LuaChangeSet(b.finish()))
            },
        )?,
    )?;

    Ok(())
}
