//! Frame userdata — Lua handle on [`gauchito_ui::Frame`].
//!
//! Exposes the three paint primitives (`text`, `fill`, `box`) plus
//! `set_cursor`, with two small Lua-side ergonomics layered on:
//!
//! - `text` accepts either a `{text, style}` run list or a bare
//!   string (one unstyled run). Simple "paint this label" callers
//!   don't pay table-construction cost.
//! - `box`'s charset is a name (`"rounded"`, `"double"`, `"ascii"`)
//!   or a custom `{tl, tr, bl, br, h, v}` table — same primitive,
//!   two ergonomics, no second method.
//!
//! Style is sparse: only fields present in the Lua table take
//! effect. Missing fields inherit (style fold per RFC §5.4).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::prelude::*;

use gauchito_ui::frame::{BoxChars, BoxCharset, CursorStyle, Frame, Run};
use gauchito_ui::{Color, Modifier, Rect, Style};

/// Handle to the substrate's persistent paint buffer.
#[derive(Clone)]
pub struct LuaFrame {
    pub frame: Rc<RefCell<Frame>>,
}

impl LuaUserData for LuaFrame {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("area", |lua, this, ()| {
            let r = this.frame.borrow().area();
            rect_to_lua(lua, r)
        });

        methods.add_method("set_area", |_, this, area: LuaTable| {
            this.frame.borrow_mut().set_area(rect_from_lua(&area)?);
            Ok(())
        });

        methods.add_method("clear", |_, this, ()| {
            this.frame.borrow_mut().clear();
            Ok(())
        });

        // text(area, runs) — single-row styled spans.
        //
        // `runs` is either a list of `{text, style}` tables (the
        // primary form), or a single string (shorthand for one
        // unstyled run). The shorthand keeps simple "paint this line"
        // callers terse.
        methods.add_method(
            "text",
            |_, this, (area, runs): (LuaTable, LuaValue)| {
                let area = rect_from_lua(&area)?;
                let runs = runs_from_lua(runs)?;
                this.frame.borrow_mut().text(area, runs);
                Ok(())
            },
        );

        methods.add_method(
            "fill",
            |_, this, (area, ch, style): (LuaTable, String, Option<LuaTable>)| {
                let area = rect_from_lua(&area)?;
                let ch = ch.chars().next().unwrap_or(' ');
                let style = style_from_lua(style)?;
                this.frame.borrow_mut().fill(area, ch, style);
                Ok(())
            },
        );

        // box(area, charset, style)
        //
        // `charset` is one of "rounded" | "double" | "ascii" or a
        // table { tl, tr, bl, br, h, v } of single chars.
        methods.add_method(
            "box",
            |_, this, (area, charset, style): (LuaTable, LuaValue, Option<LuaTable>)| {
                let area = rect_from_lua(&area)?;
                let charset = charset_from_lua(charset)?;
                let style = style_from_lua(style)?;
                this.frame.borrow_mut().box_(area, charset, style);
                Ok(())
            },
        );

        methods.add_method(
            "set_cursor",
            |_, this, (x, y, style): (u16, u16, Option<String>)| {
                let style = cursor_style_from_lua(style.as_deref());
                this.frame.borrow_mut().set_cursor(x, y, style);
                Ok(())
            },
        );
    }
}

fn runs_from_lua(value: LuaValue) -> LuaResult<Vec<Run>> {
    match value {
        // Shorthand: "hello" → one default-styled run.
        LuaValue::String(s) => Ok(vec![Run {
            text:  s.to_str()?.to_string(),
            style: Style::default(),
        }]),
        LuaValue::Table(t) => {
            let mut out = Vec::with_capacity(t.raw_len());
            for entry in t.sequence_values::<LuaValue>() {
                let entry = entry?;
                match entry {
                    // String entry: unstyled run.
                    LuaValue::String(s) => out.push(Run {
                        text:  s.to_str()?.to_string(),
                        style: Style::default(),
                    }),
                    // Table entry: { text, style } — supports both
                    // positional [1]=text [2]=style and named keys.
                    LuaValue::Table(t) => {
                        let text: String = t
                            .get::<String>(1)
                            .or_else(|_| t.get::<String>("text"))?;
                        let style_tbl: Option<LuaTable> =
                            t.get::<LuaTable>(2).ok().or_else(|| t.get("style").ok());
                        let style = style_from_lua(style_tbl)?;
                        out.push(Run { text, style });
                    }
                    _ => {
                        return Err(LuaError::runtime(
                            "frame:text run must be a string or {text, style} table",
                        ));
                    }
                }
            }
            Ok(out)
        }
        LuaValue::Nil => Ok(Vec::new()),
        _ => Err(LuaError::runtime(
            "frame:text expects a string or list of runs",
        )),
    }
}

fn charset_from_lua(value: LuaValue) -> LuaResult<BoxCharset> {
    match value {
        LuaValue::Nil => Ok(BoxCharset::Rounded),
        LuaValue::String(s) => Ok(match s.to_str()?.as_ref() {
            "rounded" => BoxCharset::Rounded,
            "double" => BoxCharset::Double,
            "ascii" => BoxCharset::Ascii,
            other => {
                return Err(LuaError::runtime(format!(
                    "frame:box charset '{other}' is not one of rounded|double|ascii"
                )));
            }
        }),
        LuaValue::Table(t) => Ok(BoxCharset::Custom(BoxChars {
            tl: first_char(&t, "tl")?,
            tr: first_char(&t, "tr")?,
            bl: first_char(&t, "bl")?,
            br: first_char(&t, "br")?,
            h:  first_char(&t, "h")?,
            v:  first_char(&t, "v")?,
        })),
        _ => Err(LuaError::runtime(
            "frame:box charset expects a name or a charset table",
        )),
    }
}

fn first_char(t: &LuaTable, key: &str) -> LuaResult<char> {
    let s: String = t.get(key)?;
    s.chars()
        .next()
        .ok_or_else(|| LuaError::runtime(format!("charset.{key} cannot be empty")))
}

fn rect_to_lua(lua: &Lua, r: Rect) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", r.x)?;
    t.set("y", r.y)?;
    t.set("w", r.width)?;
    t.set("h", r.height)?;
    Ok(t)
}

fn rect_from_lua(t: &LuaTable) -> LuaResult<Rect> {
    let x: u16 = t.get("x")?;
    let y: u16 = t.get("y")?;
    let w: u16 = t.get("w")?;
    let h: u16 = t.get("h")?;
    Ok(Rect::new(x, y, w, h))
}

fn cursor_style_from_lua(name: Option<&str>) -> CursorStyle {
    match name {
        Some("bar") => CursorStyle::Bar,
        _ => CursorStyle::Block,
    }
}

fn style_from_lua(t: Option<LuaTable>) -> LuaResult<Style> {
    let mut s = Style::default();
    let Some(t) = t else { return Ok(s) };

    if let Ok(name) = t.get::<String>("fg") {
        if let Some(c) = parse_color(&name) {
            s = s.fg(c);
        }
    }
    if let Ok(name) = t.get::<String>("bg") {
        if let Some(c) = parse_color(&name) {
            s = s.bg(c);
        }
    }
    if let Ok(true) = t.get::<bool>("bold") {
        s = s.add_modifier(Modifier::BOLD);
    }
    if let Ok(true) = t.get::<bool>("italic") {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if let Ok(true) = t.get::<bool>("underline") {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if let Ok(true) = t.get::<bool>("reverse") {
        s = s.add_modifier(Modifier::REVERSED);
    }
    Ok(s)
}

fn parse_color(name: &str) -> Option<Color> {
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
