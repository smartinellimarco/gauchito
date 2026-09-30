//! Conversion between Lua-shaped option tables and `PartialBufferOptions`.
//!
//! `gauchito.io.read` and `gauchito.buf.from_rope` both traffic in the same partial-options shape —
//! a sparse Lua table whose keys mirror `BufferOptions` field names.
//! The helpers here are the single source of truth for that mapping.

use mlua::prelude::*;

use gauchito_core::options::PartialBufferOptions;
use gauchito_core::{IndentStyle, LineEnding};

/// Build a sparse Lua table from a `PartialBufferOptions`. Unset fields
/// are omitted (so the table round-trips through `gauchito.options.merge`
/// without ghost-overriding earlier layers).
pub fn partial_to_lua(lua: &Lua, p: &PartialBufferOptions) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    if let Some(le) = p.line_ending {
        t.set(
            "line_ending",
            match le {
                LineEnding::Lf => "lf",
                LineEnding::Crlf => "crlf",
            },
        )?;
    }
    if let Some(fnl) = p.final_newline {
        t.set("final_newline", fnl)?;
    }
    if let Some(bom) = p.bom {
        t.set("bom", bom)?;
    }
    if let Some(trim) = p.trim_trailing_whitespace {
        t.set("trim_trailing_whitespace", trim)?;
    }
    if let Some(indent) = &p.indent {
        let it = lua.create_table()?;
        it.set("unit", indent.unit.as_str())?;
        it.set("tab_width", indent.tab_width)?;
        t.set("indent", it)?;
    }
    Ok(t)
}

/// Parse a Lua-shaped partial-options table into a `PartialBufferOptions`.
/// Unknown keys are silently ignored. A `nil` input is treated as an empty
/// partial.
pub fn partial_from_lua(t: Option<LuaTable>) -> LuaResult<PartialBufferOptions> {
    let Some(t) = t else {
        return Ok(PartialBufferOptions::default());
    };

    let line_ending = match t.get::<Option<String>>("line_ending")? {
        Some(s) if s == "lf" => Some(LineEnding::Lf),
        Some(s) if s == "crlf" => Some(LineEnding::Crlf),
        Some(other) => return Err(LuaError::runtime(format!("unknown line_ending: {other:?}"))),
        None => None,
    };

    let indent = match t.get::<Option<LuaTable>>("indent")? {
        Some(it) => Some(IndentStyle {
            unit:      it.get::<String>("unit")?,
            tab_width: it.get::<u8>("tab_width")?,
        }),
        None => None,
    };

    Ok(PartialBufferOptions {
        line_ending,
        final_newline:            t.get("final_newline")?,
        bom:                      t.get("bom")?,
        trim_trailing_whitespace: t.get("trim_trailing_whitespace")?,
        indent,
    })
}
