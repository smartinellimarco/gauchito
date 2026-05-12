//! Lua-side handle into the live editor state.
//!
//! `Ctx` is the userdata passed to every key handler. It holds shared handles
//! to [`EditorState`] (interior-mutated through `Rc<RefCell<…>>`) and to a
//! per-dispatch effects accumulator. Methods take the cell out via
//! `borrow_mut()` for the duration of one Lua call — never across a yield.
//!
//! The bridge is intentionally narrow: queries (`text`, `selection`, `mode`),
//! one-shot mutations (`set_selection`, `map_selections`, `edit`), mode and
//! transaction state, splits, and lifecycle effects. All motion / shape /
//! mutation logic lives in pure kernels (`bv.k.*`) and Lua combinators
//! (`bv.collapse`, `bv.fold`, …); the preset composes them.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::prelude::*;

use gauchito_core::history::SelectionSnapshot;
use gauchito_core::selection::{Range, Selection};
use gauchito_ui::{CursorStyle, EditorState, Effect, SplitDirection};

use crate::userdata::{LuaBuffer, LuaChangeSet, LuaSelection};

pub type SharedState = Rc<RefCell<EditorState>>;
pub type SharedEffects = Rc<RefCell<Vec<Effect>>>;

#[derive(Clone)]
pub struct Ctx {
    state: SharedState,
    effects: SharedEffects,
}

impl Ctx {
    pub fn new(state: SharedState, effects: SharedEffects) -> Self {
        Ctx { state, effects }
    }
}

impl LuaUserData for Ctx {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // ── Queries ─────────────────────────────────────────────────────

        methods.add_method("mode", |_, this, ()| Ok(this.state.borrow().mode.clone()));

        methods.add_method("text", |_, this, ()| {
            Ok(LuaBuffer(this.state.borrow().focused_doc().text.clone()))
        });

        methods.add_method("selection", |_, this, ()| {
            let s = this.state.borrow();
            let view = s.focused_view();
            let doc = &s.documents[&view.doc_id];
            Ok(LuaSelection(view.selection.snapshot(&doc.anchors)))
        });

        // ── Mode / cursor style ─────────────────────────────────────────

        methods.add_method("set_mode", |_, this, name: String| {
            this.state.borrow_mut().mode = name;
            Ok(())
        });

        methods.add_method("set_cursor_style", |_, this, style: String| {
            let s = match style.as_str() {
                "bar" => CursorStyle::Bar,
                _ => CursorStyle::Block,
            };
            this.state.borrow_mut().cursor_style = s;
            Ok(())
        });

        // ── Selection mutation ──────────────────────────────────────────

        methods.add_method("set_selection", |_, this, sel: LuaSelection| {
            replace_focused_selection(this, sel.0);
            Ok(())
        });

        // Convenience: map every (anchor, head) pair through a Lua function
        // that returns the new (anchor, head). Equivalent to
        // `local s = ctx:selection(); ... ; ctx:set_selection(new)`.
        methods.add_method("map_selections", |_, this, f: LuaFunction| {
            let snap = {
                let s = this.state.borrow();
                let view = s.focused_view();
                let doc = &s.documents[&view.doc_id];
                view.selection.snapshot(&doc.anchors)
            };

            let new_ranges: Vec<(usize, usize)> = snap
                .ranges
                .iter()
                .map(|&(a, h)| {
                    let result: LuaMultiValue = f.call((a, h))?;
                    let vals: Vec<LuaValue> = result.into_vec();
                    let na: usize = vals
                        .first()
                        .and_then(|v| v.as_integer())
                        .ok_or_else(|| LuaError::runtime("map_selections: anchor not int"))?
                        as usize;
                    let nh: usize = vals
                        .get(1)
                        .and_then(|v| v.as_integer())
                        .ok_or_else(|| LuaError::runtime("map_selections: head not int"))?
                        as usize;
                    Ok((na, nh))
                })
                .collect::<LuaResult<Vec<_>>>()?;

            replace_focused_selection(
                this,
                SelectionSnapshot {
                    ranges: new_ranges,
                    primary: snap.primary,
                },
            );
            Ok(())
        });

        // ── Edit ────────────────────────────────────────────────────────

        methods.add_method("edit", |_, this, cs: LuaChangeSet| {
            let mut s = this.state.borrow_mut();
            let doc_id = s.focused_doc().id;
            s.apply_edit(doc_id, cs.0);
            Ok(())
        });

        // ── History / transactions ──────────────────────────────────────

        methods.add_method("undo", |_, this, ()| {
            this.state.borrow_mut().undo();
            Ok(())
        });

        methods.add_method("redo", |_, this, ()| {
            this.state.borrow_mut().redo();
            Ok(())
        });

        methods.add_method("transaction_start", |_, this, ()| {
            let mut s = this.state.borrow_mut();
            let doc_id = s.focused_doc().id;
            let view_id = s.focused;
            s.transaction_start(doc_id, view_id);
            Ok(())
        });

        methods.add_method("transaction_commit", |_, this, ()| {
            let mut s = this.state.borrow_mut();
            let doc_id = s.focused_doc().id;
            let view_id = s.focused;
            s.transaction_commit(doc_id, view_id);
            Ok(())
        });

        // ── Multi-cursor primitives ────────────────────────────────────
        // Higher-level operations (add_cursor_below, keep_primary, …) are
        // composed in Lua on top of these.

        // Push a new range (anchor, head) onto the selection. The new range
        // becomes primary; overlapping ranges are merged automatically.
        methods.add_method("push_cursor", |_, this, (anchor, head): (usize, usize)| {
            let mut guard = this.state.borrow_mut();
            let s = &mut *guard;
            let view_id = s.focused;
            let doc_id = s.views[&view_id].doc_id;
            let doc = s.documents.get_mut(&doc_id).unwrap();
            let view = s.views.get_mut(&view_id).unwrap();
            let r = Range::new(&mut doc.anchors, anchor, head);
            view.selection.push(&mut doc.anchors, r);
            Ok(())
        });

        // Drop the range at `idx`, freeing its anchors. Removing the last
        // range is silently rejected (Selection invariant).
        methods.add_method("remove_cursor", |_, this, idx: usize| {
            let mut guard = this.state.borrow_mut();
            let s = &mut *guard;
            let view_id = s.focused;
            let doc_id = s.views[&view_id].doc_id;
            let doc = s.documents.get_mut(&doc_id).unwrap();
            let view = s.views.get_mut(&view_id).unwrap();
            if view.selection.len() > 1 && idx < view.selection.len() {
                view.selection.remove(&mut doc.anchors, idx);
            }
            Ok(())
        });

        // Promote the range at `idx` to primary.
        methods.add_method("set_primary", |_, this, idx: usize| {
            let mut s = this.state.borrow_mut();
            let view = s.focused_view_mut();
            if idx < view.selection.len() {
                view.selection.set_primary(idx);
            }
            Ok(())
        });

        // ── Splits / focus / lifecycle ──────────────────────────────────

        methods.add_method("split_horizontal", |_, this, ()| {
            this.state
                .borrow_mut()
                .split_focused(SplitDirection::Horizontal);
            Ok(())
        });

        methods.add_method("split_vertical", |_, this, ()| {
            this.state
                .borrow_mut()
                .split_focused(SplitDirection::Vertical);
            Ok(())
        });

        methods.add_method("focus_next", |_, this, ()| {
            this.state.borrow_mut().focus_next();
            Ok(())
        });

        methods.add_method("focus_prev", |_, this, ()| {
            this.state.borrow_mut().focus_prev();
            Ok(())
        });

        methods.add_method("close_view", |_, this, ()| {
            if this.state.borrow_mut().close_view() {
                this.effects.borrow_mut().push(Effect::Quit);
            }
            Ok(())
        });

        // ── Effects (deferred to app) ──────────────────────────────────

        methods.add_method("save", |_, this, ()| {
            let _ = this.state.borrow().focused_doc().write();
            Ok(())
        });

        methods.add_method("quit", |_, this, ()| {
            this.effects.borrow_mut().push(Effect::Quit);
            Ok(())
        });
    }
}

// ── Internals ───────────────────────────────────────────────────────────────

/// Replace the focused view's selection by allocating fresh anchors from `snap`.
/// Drops the previous selection's anchors so the [`AnchorTable`] doesn't leak.
fn replace_focused_selection(this: &Ctx, snap: SelectionSnapshot) {
    let mut guard = this.state.borrow_mut();
    let s = &mut *guard;
    let view_id = s.focused;
    let doc_id = s.views[&view_id].doc_id;

    let doc = s.documents.get_mut(&doc_id).unwrap();
    let new_sel = Selection::from_snapshot(&mut doc.anchors, &snap);

    let view = s.views.get_mut(&view_id).unwrap();
    let old = std::mem::replace(&mut view.selection, new_sel);
    old.drop(&mut doc.anchors);
}

