//! Async task runtime — drives Lua coroutines that await Rust events.
//!
//! # The model in one breath
//!
//! One Lua state, one thread (the main thread). Lua never crosses
//! threads. Tokio exists only to run background *work* that produces
//! plain data; it never touches Lua. The two halves meet over an mpsc
//! channel:
//!
//! - A **future** is the WORK: an off-thread job (a timer, a blocking
//!   file read) that eventually yields a value. It runs on tokio and
//!   cannot see Lua.
//! - **`await`** is the WAITING: a Lua coroutine suspends at
//!   `coroutine.yield(awaitable)` and resumes later, on the main thread.
//! - The **channel + [`TaskId`]** is the baton: a finished future sends
//!   [`TaskWake`]`{ id, result }`; the host loop pulls it and resumes the
//!   matching coroutine.
//!
//! An "awaitable" is just a Lua table `{ _kind = '…' }` whose `:await()`
//! does `coroutine.yield(self)` (see `prelude.lua`). The yield hands the
//! table up to [`drive`], which reads `_kind` and arranges the wake.
//!
//! # Two flavors of suspension
//!
//! 1. **Future-backed** (`sleep`, `io.read`): [`schedule_yield`] spawns a
//!    tokio future; on completion it sends a [`TaskWake`] and the host
//!    calls [`TaskRuntime::resume`].
//! 2. **Event-backed** (`keys.read`): no future at all — the coroutine is
//!    parked in the single `key_waiter` slot, and the host delivers the
//!    next key via [`TaskRuntime::feed_key`] as a `{code, ch, ctrl, alt,
//!    shift}` table (Lua-side naming like 'ctrl-a' is the prelude's job).
//!    Keys that arrive with no waiter queue up and wake the next
//!    `keys.read` in order, so nothing typed during startup is lost.
//!    Only one coroutine waits on keys at a time; a second `keys.read`
//!    overwrites the slot and the previous waiter is dropped. In practice
//!    the distro's top-level loop is the only reader; nested chord
//!    handlers extend it.
//!
//! # Why results are copied, not borrowed
//!
//! A future is `Send` and runs off the main thread, so it can hold
//! neither an `Rc` nor a Lua handle. It must produce *plain data*
//! ([`TaskResult`]) and ship it over the channel; only back on the main
//! thread is that data marshalled into Lua (`result_to_lua`,
//! `key_event_to_table`). This isn't waste — what crosses is mostly a
//! cheap move of freshly-created data (an Arc-shared `Rope`, a key
//! event), never a copy of a live buffer.
//!
//! The deeper rule: you cannot hold a borrow across a suspension point —
//! a thread hop *or* a coroutine yield — because suspension lets other
//! code run, and a borrow is a promise that nothing else touches the
//! value. So synchronous userdata methods borrow (cheap, shared); the
//! async seam copies. For the related single-thread re-entrant-borrow
//! case, see `userdata::buffer::borrow_mut_buf`.
//!
//! # Errors are isolated
//!
//! A task body that throws is caught in [`drive`]: re-emitted as
//! `task.error` on the Lua event bus, and not re-parked. One bad task
//! never takes down the runtime or the other tasks.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mlua::prelude::*;
use ropey::Rope;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use gauchito_core::options::PartialBufferOptions;

use crate::options::partial_to_lua;
use crate::userdata::LuaRope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

#[derive(Debug)]
pub struct TaskWake {
    pub id: TaskId,
    pub result: TaskResult,
}

#[derive(Debug)]
pub enum TaskResult {
    Nil,
    IoRead(std::io::Result<(Rope, PartialBufferOptions)>),
    Key(KeyEvent),
}

pub type TaskWakeRx = UnboundedReceiver<TaskWake>;

/// Wraps the suspension tables and the wake-channel sender. The receiver
/// is owned externally so the CLI's `tokio::select!` can borrow it
/// independently of `&mut ScriptRuntime` borrows.
pub struct TaskRuntime {
    inner: Rc<RefCell<Inner>>,
    tx: UnboundedSender<TaskWake>,
}

struct Inner {
    next_id: u64,
    threads: HashMap<TaskId, LuaRegistryKey>,
    key_waiter: Option<TaskId>,
    // Keys that arrived while no task was reading; `keys.read` drains them in order.
    pending_keys: VecDeque<KeyEvent>,
}

impl TaskRuntime {
    pub fn new() -> (Self, TaskWakeRx) {
        let (tx, rx) = unbounded_channel();
        let rt = TaskRuntime {
            inner: Rc::new(RefCell::new(Inner {
                next_id: 1,
                threads: HashMap::new(),
                key_waiter: None,
                pending_keys: VecDeque::new(),
            })),
            tx,
        };
        (rt, rx)
    }

    /// Install `gauchito.__task_spawn` (wrapped as `gauchito.task.spawn`
    /// in the prelude). Spawning runs the function in a new coroutine
    /// and drives it once; if it yields with an awaitable, we register
    /// for the appropriate wake.
    pub fn register(&self, lua: &Lua) -> LuaResult<()> {
        let inner = self.inner.clone();
        let tx = self.tx.clone();

        let gauchito: LuaTable = lua.globals().get("gauchito")?;
        gauchito.set(
            "__task_spawn",
            lua.create_function(move |lua, fn_: LuaFunction| {
                let id = next_id(&inner);
                let thread = lua.create_thread(fn_)?;
                drive(lua, &inner, &tx, id, &thread, LuaMultiValue::new())?;

                let handle = lua.create_table()?;
                handle.set("id", id.0)?;
                Ok(handle)
            })?,
        )?;
        Ok(())
    }

    /// Resume a task whose awaited future just completed.
    pub fn resume(&self, lua: &Lua, wake: TaskWake) -> LuaResult<()> {
        let key = self.inner.borrow_mut().threads.remove(&wake.id);
        let Some(key) = key else {
            return Ok(());
        };

        let thread: LuaThread = lua.registry_value(&key)?;
        lua.remove_registry_value(key)?;

        let args = result_to_lua(lua, wake.result)?;
        drive(lua, &self.inner, &self.tx, wake.id, &thread, args)
    }

    /// Deliver a terminal key event to the coroutine waiting on
    /// `gauchito.keys.read():await()`. The crossterm event is unpacked
    /// into a plain Lua table — distros never see crossterm types and
    /// naming convention ('ctrl-a', '<C-a>', …) is the prelude's job.
    pub fn feed_key(&self, lua: &Lua, event: KeyEvent) -> LuaResult<()> {
        let id = self.inner.borrow_mut().key_waiter.take();
        let Some(id) = id else {
            self.inner.borrow_mut().pending_keys.push_back(event);
            return Ok(());
        };
        let key_thread = self.inner.borrow_mut().threads.remove(&id);
        let Some(reg) = key_thread else {
            return Ok(());
        };

        let thread: LuaThread = lua.registry_value(&reg)?;
        lua.remove_registry_value(reg)?;

        let table = key_event_to_table(lua, event)?;
        let args = LuaMultiValue::from_iter([LuaValue::Table(table)]);
        drive(lua, &self.inner, &self.tx, id, &thread, args)
    }
}

fn result_to_lua(lua: &Lua, result: TaskResult) -> LuaResult<LuaMultiValue> {
    match result {
        TaskResult::Nil => Ok(LuaMultiValue::from_iter([LuaValue::Nil, LuaValue::Nil])),
        TaskResult::IoRead(Ok((rope, sniffed))) => {
            let t = lua.create_table()?;
            t.set("rope", LuaRope(rope))?;
            t.set("sniffed", partial_to_lua(lua, &sniffed)?)?;
            Ok(LuaMultiValue::from_iter([
                LuaValue::Table(t),
                LuaValue::Nil,
            ]))
        }
        TaskResult::Key(event) => {
            let table = key_event_to_table(lua, event)?;
            Ok(LuaMultiValue::from_iter([LuaValue::Table(table)]))
        }
        TaskResult::IoRead(Err(e)) => {
            let err = lua.create_string(format!("io.read: {e}"))?;
            Ok(LuaMultiValue::from_iter([
                LuaValue::Nil,
                LuaValue::String(err),
            ]))
        }
    }
}

/// Translate a crossterm `KeyEvent` into the substrate's Lua-side
/// shape: `{ code, ch, ctrl, alt, shift }`. `code` is one of the
/// substrate names — `char`, `enter`, `esc`, `backspace`, `del`, `tab`,
/// `left`/`right`/`up`/`down`, `home`/`end`, `pageup`/`pagedown`,
/// `space`, `f1`..`f12`, `unknown`. Naming convention (`ctrl-a` vs
/// `<C-a>`) is the Lua prelude's job — this carries only facts.
fn key_event_to_table(lua: &Lua, event: KeyEvent) -> LuaResult<LuaTable> {
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);

    let (code, ch): (String, Option<char>) = match event.code {
        KeyCode::Char(' ') => ("space".into(), Some(' ')),
        KeyCode::Char(c) => ("char".into(), Some(c)),
        KeyCode::Enter => ("enter".into(), None),
        KeyCode::Esc => ("esc".into(), None),
        KeyCode::Backspace => ("backspace".into(), None),
        KeyCode::Delete => ("del".into(), None),
        KeyCode::Tab => ("tab".into(), None),
        KeyCode::Left => ("left".into(), None),
        KeyCode::Right => ("right".into(), None),
        KeyCode::Up => ("up".into(), None),
        KeyCode::Down => ("down".into(), None),
        KeyCode::Home => ("home".into(), None),
        KeyCode::End => ("end".into(), None),
        KeyCode::PageUp => ("pageup".into(), None),
        KeyCode::PageDown => ("pagedown".into(), None),
        KeyCode::F(n) => (format!("f{n}"), None),
        _ => ("unknown".into(), None),
    };

    let t = lua.create_table()?;
    t.set("code", code)?;
    match ch {
        Some(c) => t.set("ch", c.to_string())?,
        None => t.set("ch", LuaValue::Nil)?,
    }
    t.set("ctrl", ctrl)?;
    t.set("alt", alt)?;
    t.set("shift", shift)?;
    Ok(t)
}

fn next_id(inner: &Rc<RefCell<Inner>>) -> TaskId {
    let mut g = inner.borrow_mut();
    let id = TaskId(g.next_id);
    g.next_id += 1;
    id
}

/// One step of a coroutine: resume with `args`, handle the outcome.
fn drive(
    lua: &Lua,
    inner: &Rc<RefCell<Inner>>,
    tx: &UnboundedSender<TaskWake>,
    id: TaskId,
    thread: &LuaThread,
    args: LuaMultiValue,
) -> LuaResult<()> {
    match thread.resume::<LuaMultiValue>(args) {
        Ok(yielded) => {
            if matches!(thread.status(), LuaThreadStatus::Resumable) {
                schedule_yield(inner, tx, id, yielded)?;
                let key = lua.create_registry_value(thread.clone())?;
                inner.borrow_mut().threads.insert(id, key);
            }
            Ok(())
        }
        Err(e) => emit_task_error(lua, id, &e.to_string()),
    }
}

fn emit_task_error(lua: &Lua, id: TaskId, message: &str) -> LuaResult<()> {
    let gauchito: LuaTable = lua.globals().get("gauchito")?;
    let event: LuaTable = gauchito.get("event")?;
    let emit: LuaFunction = event.get("emit")?;
    let payload = lua.create_table()?;
    payload.set("task", id.0)?;
    payload.set("message", message)?;
    emit.call::<()>(("task.error", payload))
}

fn schedule_yield(
    inner: &Rc<RefCell<Inner>>,
    tx: &UnboundedSender<TaskWake>,
    id: TaskId,
    yielded: LuaMultiValue,
) -> LuaResult<()> {
    let val = yielded
        .into_iter()
        .next()
        .ok_or_else(|| LuaError::runtime("task yielded with no awaitable"))?;
    let table = match val {
        LuaValue::Table(t) => t,
        _ => return Err(LuaError::runtime("task yielded a non-awaitable value")),
    };

    let kind: String = table.get("_kind")?;
    match kind.as_str() {
        "sleep" => {
            let ms: u64 = table.get("_ms")?;
            let tx = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                let _ = tx.send(TaskWake {
                    id,
                    result: TaskResult::Nil,
                });
            });
            Ok(())
        }
        "io.read" => {
            let path: String = table.get("_path")?;
            let tx = tx.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    gauchito_core::io::read(&PathBuf::from(path))
                })
                .await
                .unwrap_or_else(|e| {
                    Err(std::io::Error::other(format!(
                        "io.read worker panicked: {e}"
                    )))
                });
                let _ = tx.send(TaskWake {
                    id,
                    result: TaskResult::IoRead(result),
                });
            });
            Ok(())
        }
        "keys.read" => {
            // A queued key wakes the task through the channel like any
            // other result; otherwise it waits for the host's `feed_key`.
            let mut inner = inner.borrow_mut();
            match inner.pending_keys.pop_front() {
                Some(event) => {
                    let _ = tx.send(TaskWake {
                        id,
                        result: TaskResult::Key(event),
                    });
                }
                None => inner.key_waiter = Some(id),
            }
            Ok(())
        }
        other => Err(LuaError::runtime(format!(
            "unknown awaitable kind: {other}"
        ))),
    }
}
