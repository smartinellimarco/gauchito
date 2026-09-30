use std::cell::{Cell, RefCell, RefMut};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crossterm::terminal;
use mlua::prelude::*;
use ropey::RopeSlice;
use tokio::sync::{Mutex, Notify, mpsc};

use gauchito_core::movement::{
    CharClass, LINES, char_class, last_navigable_line, move_doc_end, move_first_non_whitespace,
    move_left, move_left_inline, move_line_end, move_line_start, move_right, move_right_inline,
    move_to_line, move_vertical, position_at, skip_class_backward, skip_class_forward,
    visual_column,
};
use gauchito_core::selection::SelectionSnapshot;
use gauchito_core::{Buffer, Selection, Splice, View};
use gauchito_ui::frame::{BoxChars, BoxCharset, CursorStyle, Frame, Run};
use gauchito_ui::{Color, Modifier, Rect, Style};

pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn user_config_path() -> PathBuf {
    gauchito_core::paths::init_file()
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

// What the tasks share with the host loop; `wake` tells the host to look at it again.
struct Host {
    frame: Rc<RefCell<Frame>>,
    quit: Cell<bool>,
    tasks: Cell<usize>,
    error: RefCell<Option<String>>,
    wake: Notify,
}

pub struct ScriptRuntime {
    lua: Lua,
    host: Rc<Host>,
    keys: mpsc::UnboundedSender<KeyEvent>,
}

impl ScriptRuntime {
    pub fn new(argv: Vec<String>) -> Result<Self, ScriptError> {
        let lua = Lua::new();
        let host = Rc::new(Host {
            frame: Rc::new(RefCell::new(Frame::new())),
            quit: Cell::new(false),
            tasks: Cell::new(0),
            error: RefCell::new(None),
            wake: Notify::new(),
        });
        let (keys, keys_rx) = mpsc::unbounded_channel();

        let gauchito = lua.create_table()?;
        gauchito.set("version", env!("CARGO_PKG_VERSION"))?;
        gauchito.set("argv", argv)?;
        gauchito.set("frame", LuaFrame(host.clone()))?;
        gauchito.set("quit", quit_fn(&lua, &host)?)?;
        gauchito.set("term", term_table(&lua)?)?;
        gauchito.set("buf", buf_table(&lua)?)?;
        gauchito.set("view", view_table(&lua)?)?;
        gauchito.set("selection", selection_table(&lua)?)?;
        gauchito.set("splice", splice_table(&lua)?)?;
        gauchito.set("task", task_table(&lua, &host)?)?;
        gauchito.set("sleep", sleep_fn(&lua)?)?;
        gauchito.set("keys", keys_table(&lua, keys_rx)?)?;
        lua.globals().set("gauchito", gauchito)?;

        Ok(ScriptRuntime { lua, host, keys })
    }

    // Must run inside a tokio LocalSet: the function the config returns is spawned as a task.
    pub fn load_config(&self, path: &Path) -> Result<(), ScriptError> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| ScriptError(format!("no config at {}: {e}", path.display())))?;
        self.start(&source)
    }

    fn start(&self, source: &str) -> Result<(), ScriptError> {
        let main: Option<LuaFunction> = self.lua.load(source).set_name("config").eval()?;
        if let Some(main) = main {
            spawn(&self.lua, &self.host, main)?;
        }
        Ok(())
    }

    pub fn feed_key(&self, event: KeyEvent) {
        let _ = self.keys.send(event);
    }

    pub async fn woken(&self) {
        self.host.wake.notified().await
    }

    pub fn error(&self) -> Option<String> {
        self.host.error.borrow_mut().take()
    }

    // Quit was asked for, or no task is left that could ever do anything.
    pub fn done(&self) -> bool {
        self.host.quit.get() || self.host.tasks.get() == 0
    }

    pub fn frame(&self) -> Rc<RefCell<Frame>> {
        self.host.frame.clone()
    }
}

fn spawn(lua: &Lua, host: &Rc<Host>, f: LuaFunction) -> LuaResult<()> {
    let task = lua.create_thread(f)?.into_async::<()>(())?;
    host.tasks.set(host.tasks.get() + 1);
    let host = host.clone();
    tokio::task::spawn_local(async move {
        if let Err(e) = task.await {
            *host.error.borrow_mut() = Some(e.to_string());
        }
        host.tasks.set(host.tasks.get() - 1);
        host.wake.notify_one();
    });
    Ok(())
}

fn task_table(lua: &Lua, host: &Rc<Host>) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    let host = host.clone();
    t.set(
        "spawn",
        lua.create_function(move |lua, f: LuaFunction| spawn(lua, &host, f))?,
    )?;
    Ok(t)
}

fn sleep_fn(lua: &Lua) -> LuaResult<LuaFunction> {
    lua.create_async_function(|_, ms: u64| async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        Ok(())
    })
}

// The receiver sits behind an async mutex so concurrent readers take keys in turn.
fn keys_table(lua: &Lua, rx: mpsc::UnboundedReceiver<KeyEvent>) -> LuaResult<LuaTable> {
    let rx = Rc::new(Mutex::new(rx));
    let t = lua.create_table()?;
    t.set(
        "read",
        lua.create_async_function(move |lua, ()| {
            let rx = rx.clone();
            async move {
                match rx.lock().await.recv().await {
                    Some(event) => key_table(&lua, event),
                    None => Err(LuaError::runtime("keys.read: the host stopped sending keys")),
                }
            }
        })?,
    )?;
    Ok(t)
}

fn key_table(lua: &Lua, event: KeyEvent) -> LuaResult<LuaTable> {
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
    t.set("ch", ch.map(String::from))?;
    t.set("ctrl", event.modifiers.contains(KeyModifiers::CONTROL))?;
    t.set("alt", event.modifiers.contains(KeyModifiers::ALT))?;
    t.set("shift", event.modifiers.contains(KeyModifiers::SHIFT))?;
    Ok(t)
}

fn quit_fn(lua: &Lua, host: &Rc<Host>) -> LuaResult<LuaFunction> {
    let host = host.clone();
    lua.create_function(move |_, ()| {
        host.quit.set(true);
        host.wake.notify_one();
        Ok(())
    })
}

fn term_table(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set(
        "size",
        lua.create_function(|lua, ()| {
            let (w, h) = terminal::size().map_err(LuaError::external)?;
            let size = lua.create_table()?;
            size.set("w", w)?;
            size.set("h", h)?;
            Ok(size)
        })?,
    )?;
    Ok(t)
}

fn buf_table(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set(
        "new",
        lua.create_function(|_, ()| Ok(LuaBuffer(Rc::new(RefCell::new(Buffer::new())))))?,
    )?;
    Ok(t)
}

fn view_table(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set(
        "new",
        lua.create_function(|_, buf: LuaUserDataRef<LuaBuffer>| {
            Ok(LuaView(Rc::new(RefCell::new(View::new(buf.0.clone())))))
        })?,
    )?;
    Ok(t)
}

fn selection_table(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set(
        "new",
        lua.create_function(|_, ranges: Vec<LuaTable>| {
            if ranges.is_empty() {
                return Err(LuaError::runtime("selection.new: at least one range required"));
            }
            let ranges = ranges
                .iter()
                .map(|r| Ok((r.get("anchor")?, r.get("head")?)))
                .collect::<LuaResult<_>>()?;
            Ok(LuaSelection(SelectionSnapshot { ranges, primary: 0 }))
        })?,
    )?;
    Ok(t)
}

fn splice_table(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set(
        "new",
        lua.create_function(|_, (p, q, text): (usize, usize, String)| {
            if p > q {
                return Err(LuaError::runtime(format!("splice.new: p ({p}) must be <= q ({q})")));
            }
            Ok(LuaSplice(Splice::new(p, q, text)))
        })?,
    )?;
    Ok(t)
}

#[derive(Clone)]
struct LuaBuffer(Rc<RefCell<Buffer>>);

impl LuaUserData for LuaBuffer {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.0.borrow().id.0));
        fields.add_field_method_get("modified", |_, this| Ok(this.0.borrow().modified));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("apply", |_, this, splices: Vec<LuaUserDataRef<LuaSplice>>| {
            let mut buf = borrow_mut(this)?;
            for s in splices {
                boundary(&buf, s.0.p())?;
                boundary(&buf, s.0.q())?;
                buf.apply(&s.0);
            }
            Ok(())
        });

        methods.add_method("len", |_, this, ()| Ok(this.0.borrow().text.len()));

        methods.add_method("line_count", |_, this, ()| {
            Ok(last_navigable_line(&this.0.borrow().text.slice(..)) + 1)
        });

        methods.add_method("char", |_, this, pos: usize| {
            let buf = this.0.borrow();
            if pos >= buf.text.len() {
                return Ok(String::new());
            }
            Ok(buf.text.char(boundary(&buf, pos)?).to_string())
        });

        methods.add_method("slice", |_, this, (from, to): (usize, usize)| {
            let buf = this.0.borrow();
            let to = boundary(&buf, to.min(buf.text.len()))?;
            let from = boundary(&buf, from.min(to))?;
            Ok(buf.text.slice(from..to).to_string())
        });

        methods.add_method("line", |_, this, n: usize| {
            let buf = this.0.borrow();
            if n >= buf.text.len_lines(LINES) {
                return Ok(String::new());
            }
            let line: String = buf.text.line(n, LINES).chars().take_while(|&c| c != '\n').collect();
            Ok(line.trim_end_matches('\r').to_string())
        });

        methods.add_method("bol", |_, this, n: usize| {
            Ok(move_to_line(&this.0.borrow().text.slice(..), n))
        });

        methods.add_method("eol", |_, this, n: usize| {
            let buf = this.0.borrow();
            let text = buf.text.slice(..);
            Ok(move_line_end(&text, move_to_line(&text, n)))
        });

        // At EOF after a final newline the cursor sits on the empty last line, not the one before.
        methods.add_method("line_at", |_, this, pos: usize| {
            let buf = this.0.borrow();
            let len = buf.text.len();
            if len == 0 {
                return Ok(0);
            }
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
            let buf = this.0.borrow();
            if pos >= buf.text.len() {
                return Ok("eol");
            }
            Ok(match char_class(buf.text.char(boundary(&buf, pos)?)) {
                CharClass::Eol => "eol",
                CharClass::Whitespace => "whitespace",
                CharClass::Word => "word",
                CharClass::Punct => "punct",
            })
        });

        add_kernel(methods, "scan_class_fwd", skip_class_forward);
        add_kernel(methods, "scan_class_bwd", skip_class_backward);
        add_kernel(methods, "left", move_left);
        add_kernel(methods, "right", move_right);
        add_kernel(methods, "left_inline", move_left_inline);
        add_kernel(methods, "right_inline", move_right_inline);
        add_kernel(methods, "up", |t, p| move_vertical(t, p, -1, None));
        add_kernel(methods, "down", |t, p| move_vertical(t, p, 1, None));
        add_kernel(methods, "line_start_of", move_line_start);
        add_kernel(methods, "line_end_of", move_line_end);
        add_kernel(methods, "first_non_ws", move_first_non_whitespace);
        add_kernel(methods, "visual_col", visual_column);

        methods.add_method("doc_end", |_, this, ()| {
            Ok(move_doc_end(&this.0.borrow().text.slice(..), 0))
        });

        methods.add_method("pos_at", |_, this, (line, col): (usize, usize)| {
            Ok(position_at(&this.0.borrow().text.slice(..), line, col))
        });
    }
}

fn add_kernel<M: LuaUserDataMethods<LuaBuffer>>(
    methods: &mut M,
    name: &str,
    kernel: fn(&RopeSlice, usize) -> usize,
) {
    methods.add_method(name, move |_, this, pos: usize| {
        let buf = this.0.borrow();
        Ok(kernel(&buf.text.slice(..), boundary(&buf, pos)?))
    });
}

// Ropey 2 panics on offsets inside a char; a script gets a Lua error instead.
fn boundary(buf: &Buffer, pos: usize) -> LuaResult<usize> {
    if pos <= buf.text.len() && buf.text.is_char_boundary(pos) {
        Ok(pos)
    } else {
        Err(LuaError::runtime(format!("position {pos} is not a char boundary")))
    }
}

// A nested edit on the same buffer is a Lua error instead of a RefCell panic.
fn borrow_mut(this: &LuaBuffer) -> LuaResult<RefMut<'_, Buffer>> {
    this.0
        .try_borrow_mut()
        .map_err(|_| LuaError::runtime("buf:apply: the buffer is already being edited"))
}

#[derive(Clone)]
struct LuaView(Rc<RefCell<View>>);

impl LuaUserData for LuaView {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.0.borrow().id.0));
        fields.add_field_method_get("buf", |_, this| Ok(LuaBuffer(this.0.borrow().buf.clone())));
        fields.add_field_method_get("selection", |_, this| {
            let view = this.0.borrow();
            let buf = view.buf.borrow();
            Ok(LuaSelection(view.selection.snapshot(&buf.pins)))
        });
    }

    // The old selection's pins go back to the buffer before the new ones are issued.
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set_selection", |_, this, sel: LuaUserDataRef<LuaSelection>| {
            let buf = this.0.borrow().buf.clone();
            let mut buf = buf.borrow_mut();
            let selection = Selection::from_snapshot(&mut buf.pins, &sel.0);
            let mut old = std::mem::replace(&mut this.0.borrow_mut().selection, selection);
            old.drop(&mut buf.pins);
            Ok(())
        });
    }
}

#[derive(Clone)]
struct LuaSelection(SelectionSnapshot);

impl LuaUserData for LuaSelection {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("ranges", |lua, this| {
            this.0
                .ranges
                .iter()
                .map(|&(anchor, head)| range_table(lua, anchor, head))
                .collect::<LuaResult<Vec<_>>>()
        });
        fields.add_field_method_get("primary", |_, this| Ok(this.0.primary));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("primary_range", |lua, this, ()| {
            let (anchor, head) = this.0.ranges[this.0.primary];
            range_table(lua, anchor, head)
        });
    }
}

fn range_table(lua: &Lua, anchor: usize, head: usize) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("anchor", anchor)?;
    t.set("head", head)?;
    Ok(t)
}

struct LuaSplice(Splice);

impl LuaUserData for LuaSplice {}

struct LuaFrame(Rc<Host>);

impl LuaFrame {
    fn paint(&self, op: impl FnOnce(&mut Frame)) {
        op(&mut self.0.frame.borrow_mut());
        self.0.wake.notify_one();
    }
}

impl LuaUserData for LuaFrame {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("area", |lua, this, ()| {
            let r = this.0.frame.borrow().area();
            let t = lua.create_table()?;
            t.set("x", r.x)?;
            t.set("y", r.y)?;
            t.set("w", r.width)?;
            t.set("h", r.height)?;
            Ok(t)
        });

        methods.add_method("set_area", |_, this, area: LuaTable| {
            let area = rect(&area)?;
            this.paint(|f| f.set_area(area));
            Ok(())
        });

        methods.add_method("clear", |_, this, ()| {
            this.paint(|f| f.clear());
            Ok(())
        });

        methods.add_method("text", |_, this, (area, runs): (LuaTable, LuaValue)| {
            let area = rect(&area)?;
            let runs = runs_from_lua(runs)?;
            this.paint(|f| f.text(area, runs));
            Ok(())
        });

        methods.add_method(
            "fill",
            |_, this, (area, ch, style): (LuaTable, String, Option<LuaTable>)| {
                let area = rect(&area)?;
                let ch = ch.chars().next().unwrap_or(' ');
                let style = style_from_lua(style)?;
                this.paint(|f| f.fill(area, ch, style));
                Ok(())
            },
        );

        methods.add_method(
            "box",
            |_, this, (area, charset, style): (LuaTable, LuaValue, Option<LuaTable>)| {
                let area = rect(&area)?;
                let charset = charset_from_lua(charset)?;
                let style = style_from_lua(style)?;
                this.paint(|f| f.box_(area, charset, style));
                Ok(())
            },
        );

        methods.add_method(
            "set_cursor",
            |_, this, (x, y, style): (u16, u16, Option<String>)| {
                let style = match style.as_deref() {
                    Some("bar") => CursorStyle::Bar,
                    _ => CursorStyle::Block,
                };
                this.paint(|f| f.set_cursor(x, y, style));
                Ok(())
            },
        );
    }
}

fn rect(t: &LuaTable) -> LuaResult<Rect> {
    Ok(Rect::new(t.get("x")?, t.get("y")?, t.get("w")?, t.get("h")?))
}

fn runs_from_lua(value: LuaValue) -> LuaResult<Vec<Run>> {
    let run = |value: LuaValue| -> LuaResult<Run> {
        match value {
            LuaValue::String(s) => Ok(Run {
                text: s.to_str()?.to_string(),
                style: Style::default(),
            }),
            LuaValue::Table(t) => Ok(Run {
                text: t.get::<String>(1).or_else(|_| t.get::<String>("text"))?,
                style: style_from_lua(t.get::<LuaTable>(2).ok().or_else(|| t.get("style").ok()))?,
            }),
            _ => Err(LuaError::runtime(
                "frame:text run must be a string or {text, style} table",
            )),
        }
    };

    match value {
        LuaValue::String(_) => Ok(vec![run(value)?]),
        LuaValue::Table(t) => t.sequence_values::<LuaValue>().map(|v| run(v?)).collect(),
        LuaValue::Nil => Ok(Vec::new()),
        _ => Err(LuaError::runtime("frame:text expects a string or list of runs")),
    }
}

fn charset_from_lua(value: LuaValue) -> LuaResult<BoxCharset> {
    let char_at = |t: &LuaTable, key: &str| -> LuaResult<char> {
        let s: String = t.get(key)?;
        s.chars()
            .next()
            .ok_or_else(|| LuaError::runtime(format!("charset.{key} cannot be empty")))
    };

    match value {
        LuaValue::Nil => Ok(BoxCharset::Rounded),
        LuaValue::String(s) => match s.to_str()?.as_ref() {
            "rounded" => Ok(BoxCharset::Rounded),
            "double" => Ok(BoxCharset::Double),
            "ascii" => Ok(BoxCharset::Ascii),
            other => Err(LuaError::runtime(format!(
                "frame:box charset '{other}' is not one of rounded|double|ascii"
            ))),
        },
        LuaValue::Table(t) => Ok(BoxCharset::Custom(BoxChars {
            tl: char_at(&t, "tl")?,
            tr: char_at(&t, "tr")?,
            bl: char_at(&t, "bl")?,
            br: char_at(&t, "br")?,
            h: char_at(&t, "h")?,
            v: char_at(&t, "v")?,
        })),
        _ => Err(LuaError::runtime(
            "frame:box charset expects a name or a charset table",
        )),
    }
}

fn style_from_lua(t: Option<LuaTable>) -> LuaResult<Style> {
    let mut s = Style::default();
    let Some(t) = t else { return Ok(s) };

    if let Some(c) = t.get::<Option<String>>("fg")?.as_deref().and_then(color) {
        s = s.fg(c);
    }
    if let Some(c) = t.get::<Option<String>>("bg")?.as_deref().and_then(color) {
        s = s.bg(c);
    }
    for (key, modifier) in [
        ("bold", Modifier::BOLD),
        ("italic", Modifier::ITALIC),
        ("underline", Modifier::UNDERLINED),
        ("reverse", Modifier::REVERSED),
    ] {
        if t.get::<Option<bool>>(key)? == Some(true) {
            s = s.add_modifier(modifier);
        }
    }
    Ok(s)
}

fn color(name: &str) -> Option<Color> {
    Some(match name {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        "reset" => Color::Reset,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<F: Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(tokio::task::LocalSet::new().run_until(f))
    }

    // Lets the tasks run until nothing has woken the host for a while.
    async fn settle(rt: &ScriptRuntime) {
        while tokio::time::timeout(Duration::from_millis(20), rt.woken()).await.is_ok() {}
    }

    fn eval<T: FromLuaMulti>(rt: &ScriptRuntime, source: &str) -> T {
        rt.lua.load(source).eval().unwrap()
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    const EDITOR: &str = r#"
        buf = gauchito.buf.new()
        local view = gauchito.view.new(buf)
        return function()
            while true do
                local key = gauchito.keys.read()
                local head = view.selection:primary_range().head
                if key.code == 'char' then
                    buf:apply({ gauchito.splice.new(head, head, key.ch) })
                elseif key.code == 'backspace' then
                    buf:apply({ gauchito.splice.new(buf:left(head), head, '') })
                elseif key.code == 'left' then
                    local p = buf:left(head)
                    view:set_selection(gauchito.selection.new({ { anchor = p, head = p } }))
                elseif key.code == 'esc' then
                    gauchito.quit()
                end
            end
        end
    "#;

    #[test]
    fn a_missing_config_is_an_error() {
        let rt = ScriptRuntime::new(vec![]).unwrap();
        let err = rt.load_config(Path::new("/nonexistent/init.lua")).unwrap_err();
        assert!(err.0.starts_with("no config at /nonexistent/init.lua"), "{err}");
    }

    #[test]
    fn an_empty_config_is_done_right_away() {
        run(async {
            let rt = ScriptRuntime::new(vec![]).unwrap();
            rt.start("").unwrap();
            assert!(rt.done());
        })
    }

    #[test]
    fn the_config_sees_argv_and_version() {
        let rt = ScriptRuntime::new(vec!["--version".into()]).unwrap();
        assert_eq!(eval::<String>(&rt, "return gauchito.argv[1]"), "--version");
        assert_eq!(eval::<String>(&rt, "return gauchito.version"), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn keys_edit_the_buffer_through_the_returned_task() {
        run(async {
            let rt = ScriptRuntime::new(vec![]).unwrap();
            rt.start(EDITOR).unwrap();
            for c in "añb".chars() {
                rt.feed_key(key(c));
            }
            rt.feed_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
            rt.feed_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            settle(&rt).await;
            assert_eq!(eval::<String>(&rt, "return buf:slice(0, buf:len())"), "ab");
            assert!(!rt.done());

            rt.feed_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            settle(&rt).await;
            assert!(rt.done());
        })
    }

    #[test]
    fn keys_sent_before_anyone_reads_are_kept_in_order() {
        run(async {
            let rt = ScriptRuntime::new(vec![]).unwrap();
            for c in "abc".chars() {
                rt.feed_key(key(c));
            }
            rt.start(EDITOR).unwrap();
            settle(&rt).await;
            assert_eq!(eval::<String>(&rt, "return buf:slice(0, buf:len())"), "abc");
        })
    }

    #[test]
    fn tasks_interleave_while_they_wait() {
        run(async {
            let rt = ScriptRuntime::new(vec![]).unwrap();
            rt.start(
                r#"
                log = {}
                gauchito.task.spawn(function()
                    gauchito.sleep(10)
                    table.insert(log, 'slow')
                end)
                gauchito.task.spawn(function()
                    table.insert(log, 'fast')
                end)
                "#,
            )
            .unwrap();
            settle(&rt).await;
            assert_eq!(eval::<String>(&rt, "return table.concat(log, ',')"), "fast,slow");
            assert!(rt.done());
        })
    }

    #[test]
    fn a_failing_task_reports_its_error() {
        run(async {
            let rt = ScriptRuntime::new(vec![]).unwrap();
            rt.start("return function() error('boom') end").unwrap();
            settle(&rt).await;
            assert!(rt.error().unwrap().contains("boom"));
            assert!(rt.done());
        })
    }

    #[test]
    fn kernels_walk_multibyte_text_by_byte_offsets() {
        let rt = ScriptRuntime::new(vec![]).unwrap();
        eval::<()>(
            &rt,
            r#"
            local buf = gauchito.buf.new()
            buf:apply({ gauchito.splice.new(0, 0, "ñandú añejo") })
            assert(buf:scan_class_fwd(0) == 7)
            assert(buf:right(7) == 8)
            assert(buf:right(8) == 9)
            assert(buf:left(11) == 9)
            assert(buf:scan_class_bwd(11) == 8)
            assert(buf:visual_col(11) == 8)
            assert(buf.modified)
            "#,
        );
    }

    #[test]
    fn positions_inside_a_char_are_a_lua_error() {
        let rt = ScriptRuntime::new(vec![]).unwrap();
        let err = rt
            .lua
            .load(
                r#"
                local buf = gauchito.buf.new()
                buf:apply({ gauchito.splice.new(0, 0, "ñ") })
                buf:apply({ gauchito.splice.new(1, 1, "x") })
                "#,
            )
            .exec()
            .unwrap_err();
        assert!(err.to_string().contains("not a char boundary"), "{err}");
    }

    #[test]
    fn frame_takes_runs_fills_and_boxes() {
        let rt = ScriptRuntime::new(vec![]).unwrap();
        eval::<()>(
            &rt,
            r#"
            local f = gauchito.frame
            f:set_area({ x = 0, y = 0, w = 20, h = 5 })
            f:text({ x = 0, y = 0, w = 20, h = 1 }, 'plain')
            f:text({ x = 0, y = 1, w = 20, h = 1 }, { 'a', { 'b', { fg = 'red', bold = true } } })
            f:fill({ x = 0, y = 2, w = 20, h = 1 }, '-', { bg = 'blue' })
            f:box({ x = 0, y = 0, w = 20, h = 5 }, 'double')
            f:box({ x = 0, y = 0, w = 20, h = 5 }, { tl = '1', tr = '2', bl = '3', br = '4', h = '5', v = '6' })
            f:set_cursor(1, 1, 'bar')
            local a = f:area()
            assert(a.w == 20 and a.h == 5)
            "#,
        );
        assert!(rt.lua.load("gauchito.frame:box({ x = 0, y = 0, w = 2, h = 2 }, 'wavy')").exec().is_err());
    }
}
