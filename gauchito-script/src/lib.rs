//! Lua bridge between editor state and user config.
//!
//! `EditorState` is wrapped in `Rc<RefCell<…>>`. The bridge passes a clone of
//! that handle to a [`Ctx`] userdata for the duration of every dispatch and
//! never holds a Rust borrow across a Lua yield. There is one source of truth
//! — no snapshot, no replay.
//!
//! Multi-key sequences run as Lua coroutines. A handler that needs another
//! key calls `bv.read_key()` (= `coroutine.yield()` under the hood); the
//! bridge returns to its caller, then on the next dispatch resumes the same
//! coroutine with `(ctx, key, ch)`. Sequences end when the coroutine returns.
//! Errors and `esc` cancel cleanly.
//!
//! Per-key dispatch:
//! - direct handler under `modes[mode].keys[name]` → call `f(ctx)`
//! - else `__fallback` under `modes[mode].keys.__fallback` → call `f(ctx, name, ch)`
//! - else no-op (no implicit insert; presets opt in via `__fallback`)

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use mlua::prelude::*;

mod ctx;
mod kernels;
mod userdata;

pub use ctx::SharedState;
use ctx::{Ctx, SharedEffects};
use gauchito_ui::{Component, ComponentRegistry, EditorState, Effect};

const PRELUDE: &str = include_str!("../lua/prelude.lua");

pub struct ScriptRuntime {
    lua: Lua,
    active_thread: Option<LuaRegistryKey>,
}

impl ScriptRuntime {
    pub fn new() -> Result<Self, ScriptError> {
        let lua = Lua::new();

        kernels::register(&lua)?;
        register_ui_constructors(&lua)?;
        lua.load(PRELUDE).set_name("prelude").exec()?;

        Ok(ScriptRuntime {
            lua,
            active_thread: None,
        })
    }

    // ── Config loading ──────────────────────────────────────────────────

    pub fn load_config(&mut self, source: &str) -> Result<(), ScriptError> {
        let value: LuaValue = self.lua.load(source).set_name("config").eval()?;
        let table = match value {
            LuaValue::Table(t) => t,
            other => {
                return Err(ScriptError(format!(
                    "config must return a table, got {}",
                    other.type_name()
                )));
            }
        };
        self.lua.globals().raw_set("__modes_config", table)?;
        Ok(())
    }

    pub fn load_user_config(&mut self, path: &Path) -> Result<(), ScriptError> {
        if !path.exists() {
            return Ok(());
        }
        let source = std::fs::read_to_string(path)
            .map_err(|e| ScriptError(format!("read {}: {e}", path.display())))?;
        self.load_config(&source)
    }

    pub fn initial_mode(&self) -> String {
        let config: LuaTable = match self.lua.globals().raw_get("__modes_config") {
            Ok(t) => t,
            Err(_) => return "normal".to_string(),
        };
        config
            .get::<String>("initial_mode")
            .unwrap_or_else(|_| "normal".to_string())
    }

    pub fn component_registry(&self) -> ComponentRegistry {
        let mut registry = ComponentRegistry::new();

        let config: LuaTable = match self.lua.globals().raw_get("__modes_config") {
            Ok(t) => t,
            Err(_) => return registry,
        };
        let ui: LuaTable = match config.get("ui") {
            Ok(t) => t,
            Err(_) => return registry,
        };

        for entry in ui.sequence_values::<LuaTable>().flatten() {
            if let Ok(name) = entry.get::<String>("__component") {
                match name.as_str() {
                    "statusline" => registry.components.push(Component::Statusline),
                    other => tracing::warn!("unknown ui component: {other}"),
                }
            }
        }

        registry
    }

    pub fn run_initial_mode_callback(&mut self, _state: &SharedState) {
        // Reserved for an `on_init` hook; not wired yet.
    }

    // ── Dispatch ────────────────────────────────────────────────────────

    pub fn dispatch_key(
        &mut self,
        key_name: &str,
        ch: Option<char>,
        state: &SharedState,
    ) -> Vec<Effect> {
        let effects: SharedEffects = Rc::new(RefCell::new(Vec::new()));

        if let Err(e) = self.run(key_name, ch, state, &effects) {
            tracing::warn!("lua dispatch {key_name}: {e}");
            self.cancel();
        }

        // The Ctx userdata still holds a clone of the Rc, so try_unwrap would
        // fail. Drain in place.
        let mut bucket = effects.borrow_mut();
        std::mem::take(&mut *bucket)
    }

    fn run(
        &mut self,
        key_name: &str,
        ch: Option<char>,
        state: &SharedState,
        effects: &SharedEffects,
    ) -> LuaResult<()> {
        // Esc cancels any in-flight sequence with no further dispatch.
        if self.active_thread.is_some() && key_name == "esc" {
            self.cancel();
            return Ok(());
        }

        let ctx = Ctx::new(state.clone(), effects.clone());
        let key_lua = self.lua.create_string(key_name)?;
        let ch_lua: LuaValue = match ch {
            Some(c) => LuaValue::String(self.lua.create_string(&c.to_string())?),
            None => LuaValue::Nil,
        };

        // Resume an in-flight coroutine.
        if let Some(thread_key) = self.active_thread.as_ref() {
            let thread: LuaThread = self.lua.registry_value(thread_key)?;
            thread.resume::<LuaMultiValue>((ctx, key_lua, ch_lua))?;
            if !matches!(thread.status(), LuaThreadStatus::Resumable) {
                self.clear_thread();
            }
            return Ok(());
        }

        // No active coroutine: look up a handler for this key.
        let mode = state.borrow().mode.clone();
        let lookup = self.lookup_handler(&mode, key_name)?;

        let Some((handler, kind)) = lookup else {
            return Ok(());
        };

        // Start a coroutine. Direct handlers receive `(ctx)`; fallback handlers
        // also receive `(key_name, ch)` so they can decide what to do.
        let thread = self.lua.create_thread(handler)?;
        match kind {
            HandlerKind::Direct => {
                thread.resume::<LuaMultiValue>(ctx)?;
            }
            HandlerKind::Fallback => {
                thread.resume::<LuaMultiValue>((ctx, key_lua, ch_lua))?;
            }
        }
        if matches!(thread.status(), LuaThreadStatus::Resumable) {
            let key = self.lua.create_registry_value(thread)?;
            self.active_thread = Some(key);
        }
        Ok(())
    }

    fn cancel(&mut self) {
        self.clear_thread();
    }

    fn clear_thread(&mut self) {
        if let Some(key) = self.active_thread.take() {
            self.lua.remove_registry_value(key).ok();
        }
    }

    fn lookup_handler(
        &self,
        mode: &str,
        name: &str,
    ) -> LuaResult<Option<(LuaFunction, HandlerKind)>> {
        let config: LuaTable = match self.lua.globals().raw_get("__modes_config") {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let modes: LuaTable = match config.get("modes") {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let mt: LuaTable = match modes.get(mode) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let keys: LuaTable = match mt.get("keys") {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        if let Ok(f) = keys.get::<LuaFunction>(name) {
            return Ok(Some((f, HandlerKind::Direct)));
        }
        if let Ok(f) = keys.get::<LuaFunction>("__fallback") {
            return Ok(Some((f, HandlerKind::Fallback)));
        }
        Ok(None)
    }
}

#[derive(Clone, Copy)]
enum HandlerKind {
    /// Called as `f(ctx)`.
    Direct,
    /// Called as `f(ctx, key_name, ch)`.
    Fallback,
}

// ── UI constructors ─────────────────────────────────────────────────────────

fn register_ui_constructors(lua: &Lua) -> LuaResult<()> {
    let ui = lua.create_table()?;

    ui.set(
        "statusline",
        lua.create_function(|lua, params: Option<LuaTable>| {
            let t = lua.create_table()?;
            t.set("__component", "statusline")?;
            if let Some(p) = params {
                t.set("params", p)?;
            }
            Ok(t)
        })?,
    )?;

    let gauchito = lua.create_table()?;
    gauchito.set("ui", ui)?;
    lua.globals().set("gauchito", gauchito)?;

    Ok(())
}

// ── Public helpers ──────────────────────────────────────────────────────────

pub fn shared(state: EditorState) -> SharedState {
    Rc::new(RefCell::new(state))
}

#[derive(Debug)]
pub struct ScriptError(pub String);

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScriptError {}

impl From<LuaError> for ScriptError {
    fn from(e: LuaError) -> Self {
        ScriptError(e.to_string())
    }
}
