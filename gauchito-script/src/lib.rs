//! Lua runtime — bridges the gauchito substrate to a distro Lua program.
//!
//! Three responsibilities:
//!
//! 1. Wire substrate primitives into Lua (buffers, views, splices,
//!    selections, the frame).
//! 2. Drive coroutines: dispatch each terminal key into the coroutine
//!    waiting on `gauchito.keys.read()`, resume task wakes from tokio
//!    futures.
//! 3. Own the persistent paint buffer the CLI flushes after each event.
//!
//! No `Host`, no `focused_view`, no `Ctx`, no modes-table convention.
//! Routing, layout, scope/mode, scroll, cursor style, splits, tabs —
//! everything above selection state is distro Lua.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crossterm::terminal;
use mlua::prelude::*;

mod namespaces;
mod options;
mod task;
mod userdata;

pub use task::{TaskWake, TaskWakeRx};
// Re-exported so distro hosts (the CLI, tests) can forward terminal
// events without depending on crossterm directly.
pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Path to the distro's init.lua. Re-exported so a host doesn't need
/// to depend on `gauchito-core` just to find it.
pub fn user_config_path() -> PathBuf {
    gauchito_core::paths::init_file()
}
use task::TaskRuntime;
use userdata::LuaFrame;

use gauchito_ui::Frame;

const PRELUDE: &str = include_str!("../lua/prelude.luau");
const DEFAULT_CONFIG: &str = include_str!("../lua/presets/default.luau");

pub struct ScriptRuntime {
    lua: Lua,
    tasks: TaskRuntime,
    frame: Rc<RefCell<Frame>>,
    should_quit: Rc<RefCell<bool>>,
}

impl ScriptRuntime {
    /// Build the runtime and bind `gauchito.argv`. Returns the
    /// wake-channel receiver the host loop awaits.
    ///
    /// Frame area is left empty — distros call `frame:set_area(...)`
    /// from their first `redraw()`, typically using `gauchito.term.size()`.
    pub fn new(argv: Vec<String>) -> Result<(Self, TaskWakeRx), ScriptError> {
        let lua = Lua::new();

        // Empty `gauchito` table so prelude + later registration can
        // hang things off it.
        let gauchito = lua.create_table()?;
        gauchito.set("version", env!("CARGO_PKG_VERSION"))?;
        lua.globals().set("gauchito", gauchito)?;

        let (tasks, task_rx) = TaskRuntime::new();
        tasks.register(&lua)?;

        let frame = Rc::new(RefCell::new(Frame::new()));
        let should_quit = Rc::new(RefCell::new(false));

        register_frame(&lua, &frame)?;
        register_quit(&lua, &should_quit)?;
        register_term(&lua)?;
        register_buf(&lua)?;
        register_view(&lua)?;
        register_splice(&lua)?;
        register_selection(&lua)?;

        bind_argv(&lua, &argv)?;

        lua.load(PRELUDE).set_name("prelude").exec()?;

        // Sandbox AFTER the trusted substrate (prelude) is in
        // place: Luau freezes the std libs + globals read-only as the safe
        // baseline, then the untrusted distro config loads on top.
        lua.sandbox(true)?;

        Ok((
            ScriptRuntime {
                lua,
                tasks,
                frame,
                should_quit,
            },
            task_rx,
        ))
    }

    pub fn frame(&self) -> Rc<RefCell<Frame>> {
        self.frame.clone()
    }

    pub fn should_quit(&self) -> bool {
        *self.should_quit.borrow()
    }

    /// Load the distro's init.lua as the entry point, or the default
    /// preset when there is none. The config is responsible for spawning
    /// a coroutine that loops on `gauchito.keys.read():await()`.
    pub fn load_config(&mut self, path: &Path) -> Result<(), ScriptError> {
        if !path.exists() {
            self.lua.load(DEFAULT_CONFIG).set_name("default").exec()?;
            return Ok(());
        }
        let source = std::fs::read_to_string(path)
            .map_err(|e| ScriptError(format!("read {}: {e}", path.display())))?;
        self.lua.load(&source).set_name("config").exec()?;
        Ok(())
    }

    /// Evaluate a Lua expression. Useful for tests and a `:lua` REPL.
    pub fn eval<T: FromLuaMulti>(&self, source: &str) -> Result<T, ScriptError> {
        Ok(self.lua.load(source).set_name("eval").eval()?)
    }

    /// Deliver a key event to the coroutine waiting on
    /// `gauchito.keys.read()`. No-op if no coroutine is waiting.
    pub fn feed_key(&mut self, event: KeyEvent) -> Result<(), ScriptError> {
        Ok(self.tasks.feed_key(&self.lua, event)?)
    }

    pub fn resume_task(&mut self, wake: TaskWake) -> Result<(), ScriptError> {
        Ok(self.tasks.resume(&self.lua, wake)?)
    }
}

// ── gauchito.frame — the LuaFrame userdata itself, not an accessor fn ─────

fn register_frame(lua: &Lua, frame: &Rc<RefCell<Frame>>) -> LuaResult<()> {
    let gauchito = ensure_gauchito_table(lua)?;
    let lua_frame = LuaFrame {
        frame: frame.clone(),
    };
    gauchito.set("frame", lua_frame)?;
    Ok(())
}

// ── gauchito.argv ──────────────────────────────────────────────────────────

fn bind_argv(lua: &Lua, argv: &[String]) -> LuaResult<()> {
    let t = lua.create_table()?;
    for (i, arg) in argv.iter().enumerate() {
        t.set(i + 1, arg.as_str())?;
    }
    ensure_gauchito_table(lua)?.set("argv", t)?;
    Ok(())
}

// ── gauchito.term.size() ───────────────────────────────────────────────────
//
// Distros call this from their `redraw()` to know how big to paint.
// Pull-based so resizes "just work" without the host having to forward
// SIGWINCH events.

fn register_term(lua: &Lua) -> LuaResult<()> {
    let term = lua.create_table()?;
    term.set(
        "size",
        lua.create_function(|lua, ()| {
            let (w, h) = terminal::size().map_err(|e| LuaError::runtime(e.to_string()))?;
            let t = lua.create_table()?;
            t.set("w", w)?;
            t.set("h", h)?;
            Ok(t)
        })?,
    )?;
    ensure_gauchito_table(lua)?.set("term", term)?;
    Ok(())
}

// ── gauchito.quit() ─────────────────────────────────────────────────────────

fn register_quit(lua: &Lua, should_quit: &Rc<RefCell<bool>>) -> LuaResult<()> {
    let flag = should_quit.clone();
    let gauchito = ensure_gauchito_table(lua)?;
    gauchito.set(
        "quit",
        lua.create_function(move |_, ()| {
            *flag.borrow_mut() = true;
            Ok(())
        })?,
    )?;
    Ok(())
}

// ── gauchito.buf.{from_rope, scratch} ───────────────────────────────────────

fn register_buf(lua: &Lua) -> LuaResult<()> {
    use gauchito_core::{Buffer, BufferOptions};
    use userdata::{LuaBuffer, LuaRope};

    let buf_tbl = lua.create_table()?;

    buf_tbl.set(
        "from_rope",
        lua.create_function(
            |_lua, (rope, path, opts): (LuaRope, Option<String>, Option<LuaTable>)| {
                let partial = options::partial_from_lua(opts)?;
                let resolved = partial.resolve(BufferOptions::default());
                let path_buf = path.as_ref().map(std::path::PathBuf::from);
                let buffer = Buffer::new(rope.0, path_buf, resolved);
                Ok(LuaBuffer {
                    buf: Rc::new(RefCell::new(buffer)),
                })
            },
        )?,
    )?;

    buf_tbl.set(
        "scratch",
        lua.create_function(|_lua, (path, opts): (Option<String>, Option<LuaTable>)| {
            let partial = options::partial_from_lua(opts)?;
            let resolved = partial.resolve(BufferOptions::default());
            let buffer = Buffer::new(ropey::Rope::new(), path.map(PathBuf::from), resolved);
            Ok(LuaBuffer {
                buf: Rc::new(RefCell::new(buffer)),
            })
        })?,
    )?;

    let gauchito = ensure_gauchito_table(lua)?;
    gauchito.set("buf", buf_tbl)?;
    Ok(())
}

// ── gauchito.view.new ───────────────────────────────────────────────────────

fn register_view(lua: &Lua) -> LuaResult<()> {
    let t = namespaces::register_view(lua)?;
    let gauchito = ensure_gauchito_table(lua)?;
    gauchito.set("view", t)?;
    Ok(())
}

// ── gauchito.splice.new ─────────────────────────────────────────────────────

fn register_splice(lua: &Lua) -> LuaResult<()> {
    let t = namespaces::register_splice(lua)?;
    let gauchito = ensure_gauchito_table(lua)?;
    gauchito.set("splice", t)?;
    Ok(())
}

// ── gauchito.selection.{new, point, range} ──────────────────────────────────

fn register_selection(lua: &Lua) -> LuaResult<()> {
    let t = namespaces::register_selection(lua)?;
    let gauchito = ensure_gauchito_table(lua)?;
    gauchito.set("selection", t)?;
    Ok(())
}

fn ensure_gauchito_table(lua: &Lua) -> LuaResult<LuaTable> {
    match lua.globals().get::<LuaValue>("gauchito")? {
        LuaValue::Table(t) => Ok(t),
        _ => {
            let t = lua.create_table()?;
            lua.globals().set("gauchito", t.clone())?;
            Ok(t)
        }
    }
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
