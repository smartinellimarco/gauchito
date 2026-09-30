//! Smoke tests for the new paint regime.
//!
//! Exercises `gauchito.frame.{text,fill,box,set_cursor,clear,area}` and
//! `gauchito.paint.{buffer,view,popup,cursor}` to catch missing fields,
//! and signature drift.

use gauchito_script::ScriptRuntime;

fn make() -> ScriptRuntime {
    ScriptRuntime::new(vec![]).unwrap().0
}

#[test]
fn frame_is_a_value_not_a_function_call() {
    let rt = make();
    rt.eval::<()>(
        r#"
        assert(type(gauchito.frame) == 'userdata',
            'gauchito.frame must be a userdata value, not a function')
        local a = gauchito.frame:area()
        assert(type(a) == 'table' and a.x ~= nil and a.w ~= nil)
    "#,
    )
    .unwrap();
}

#[test]
fn frame_text_accepts_string_shorthand_and_run_list() {
    let rt = make();
    rt.eval::<()>(
        r#"
        gauchito.frame:clear()
        gauchito.frame:text({x=0,y=0,w=10,h=1}, "hello")
        gauchito.frame:text({x=0,y=1,w=10,h=1}, {
            {"hello", {fg='red'}},
            {" ",     {}},
            {"world", {bold=true, fg='blue'}},
        })
    "#,
    )
    .unwrap();
}

#[test]
fn frame_fill_and_box_compose_for_popup() {
    let rt = make();
    rt.eval::<()>(
        r#"
        gauchito.frame:clear()
        local area = {x=2, y=2, w=20, h=8}
        local inner = gauchito.paint.popup(area, { border = true })
        assert(inner.x == 3 and inner.y == 3 and inner.w == 18 and inner.h == 6,
            'popup should return the inner rect (1-cell border)')
    "#,
    )
    .unwrap();
}

#[test]
fn frame_box_accepts_named_charsets_and_custom_table() {
    let rt = make();
    rt.eval::<()>(
        r#"
        local a = {x=0,y=0,w=5,h=3}
        gauchito.frame:clear()
        gauchito.frame:box(a, 'rounded', {fg='white'})
        gauchito.frame:box(a, 'ascii', {})
        gauchito.frame:box(a, {tl='*', tr='*', bl='*', br='*', h='-', v='|'}, {})
    "#,
    )
    .unwrap();
}

#[test]
fn paint_buffer_with_view_renders_selection() {
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        buf:apply({ gauchito.splice.new(0, 0, "hello world") })
        local view = gauchito.view.new(buf)
        -- Move head to position 5 to make a 0..5 selection.
        local sel0 = view.selection
        view:set_selection(gauchito.selection.new({{anchor=0, head=5}}))

        gauchito.frame:clear()
        gauchito.paint.buffer(buf, {x=0,y=0,w=20,h=1}, { view = view })
    "#,
    )
    .unwrap();
}

#[test]
fn viewport_constructor_fills_defaults() {
    let rt = make();
    rt.eval::<()>(
        r#"
        local v = gauchito.viewport.new {}
        assert(v.scroll_row == 0 and v.scroll_col == 0)
        assert(v.show_cursor == true)
        assert(v.show_selection == true)
        assert(v.cursor_style == 'block')

        local v2 = gauchito.viewport.new { scroll_row = 5, show_selection = false }
        assert(v2.scroll_row == 5)
        assert(v2.show_selection == false)
    "#,
    )
    .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn paint_buffer_via_buf_open_with_realistic_terminal() {
    // Replicates the user's actual freeze report:
    //   terminal area ~80x24 (typical), file=vim.lua via buf.open,
    //   passing through the distro's redraw -> paint.view -> paint.buffer
    //   call shape (not the test's direct call).
    // Lines in vim.lua are well over 80 chars in places (the indent
    // helper lines, the runs entries). If line_text:sub clipping
    // produces invalid UTF-8 or trips the runs serializer, this catches.
    use gauchito_script::ScriptRuntime;
    use gauchito_ui::Rect;
    let (rt, mut rx) = ScriptRuntime::new(vec![]).unwrap();
    // Production sets the area before the user config loads.
    rt.frame().borrow_mut().set_area(Rect::new(0, 0, 80, 24));
    let rt = std::cell::RefCell::new(rt);

    let preset_path = std::env::current_dir()
        .unwrap()
        .join("..")
        .join("gauchito-script")
        .join("lua")
        .join("prelude.luau");
    let path_str = preset_path.to_string_lossy().into_owned();

    rt.borrow().eval::<()>(&format!(
        r#"
        _done = false
        _err = nil
        gauchito.task.spawn(function()
            local ok, err = pcall(function()
                local buf = gauchito.buf.open({:?}):await()
                local view = gauchito.view.new(buf)
                -- Mirror the distro: content_area() shape (one row reserved
                -- for the statusline).
                local a = gauchito.frame:area()
                local content = {{x = a.x, y = a.y, w = a.w, h = math.max(0, a.h - 1)}}
                gauchito.frame:clear()
                gauchito.paint.view(view, content, {{
                    scroll_row = 0, show_cursor = true, show_selection = true,
                    cursor_style = 'block',
                }})
            end)
            if not ok then _err = tostring(err) end
            _done = true
        end)
    "#,
        path_str
    )).unwrap();

    for _ in 0..50 {
        if rt.borrow().eval::<bool>("return _done").unwrap() { break; }
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Some(wake)) => rt.borrow_mut().resume_task(wake).unwrap(),
            _ => break,
        }
    }
    let err: Option<String> = rt.borrow().eval::<Option<String>>("return _err").unwrap();
    assert_eq!(err, None, "paint via buf.open with realistic terminal errored: {:?}", err);
}

#[tokio::test(flavor = "current_thread")]
async fn paint_buffer_via_buf_open_renders_without_nil_runs() {
    // Mirrors the actual production codepath: gauchito.buf.open reads
    // from disk and merges options. If something in that path leaves
    // a buffer in a state that breaks paint, this catches it.
    use gauchito_script::ScriptRuntime;
    let (rt, mut rx) = ScriptRuntime::new(vec![]).unwrap();
    let rt = std::cell::RefCell::new(rt);

    let preset_path = std::env::current_dir()
        .unwrap()
        .join("..")
        .join("gauchito-script")
        .join("lua")
        .join("prelude.luau");
    let path_str = preset_path.to_string_lossy().into_owned();

    rt.borrow().eval::<()>(&format!(
        r#"
        _done = false
        _err = nil
        gauchito.task.spawn(function()
            local ok, buf_or_err = pcall(function()
                return gauchito.buf.open({:?}):await()
            end)
            if not ok then _err = tostring(buf_or_err); _done = true; return end
            local buf = buf_or_err

            local view = gauchito.view.new(buf)
            gauchito.frame:clear()
            local ok2, e2 = pcall(function()
                gauchito.paint.view(view, {{x=0, y=0, w=200, h=60}}, {{
                    scroll_row = 0, show_cursor = true, show_selection = true,
                }})
            end)
            if not ok2 then _err = tostring(e2) end
            _done = true
        end)
    "#,
        path_str
    )).unwrap();

    // Pump the runtime until the task completes.
    for _ in 0..50 {
        if rt.borrow().eval::<bool>("return _done").unwrap() {
            break;
        }
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Some(wake)) => rt.borrow_mut().resume_task(wake).unwrap(),
            _ => break,
        }
    }
    let err: Option<String> = rt.borrow().eval::<Option<String>>("return _err").unwrap();
    assert_eq!(err, None, "paint via buf.open errored: {:?}", err);
}

#[test]
fn paint_buffer_renders_a_real_lua_file_without_nil_runs() {
    // Reproduces the user's freeze: open the vim.lua preset, create a
    // view, call paint.view. The previous report was
    //   "error converting Lua nil to String (expected string or number)"
    // at prelude.lua:356 (frame:text). If a nil sneaks into the runs
    // list, this should also fail.
    let rt = make();
    let preset_path = std::env::current_dir()
        .unwrap()
        .join("..")
        .join("gauchito-script")
        .join("lua")
        .join("prelude.luau");
    let preset_text = std::fs::read_to_string(&preset_path).unwrap();

    rt.eval::<()>(&format!(
        r#"
        local buf = gauchito.buf.scratch()
        buf:apply({{ gauchito.splice.new(0, 0, [==[{}]==]) }})
        local view = gauchito.view.new(buf)
        gauchito.frame:clear()
        gauchito.paint.view(view, {{x=0, y=0, w=200, h=60}}, {{
            scroll_row = 0, show_cursor = true, show_selection = true,
        }})
    "#,
        preset_text.replace("]==]", "]==[==")
    ))
    .unwrap();
}

#[test]
fn paint_buffer_handles_multibyte_lines_clipped_to_area_width() {
    // Regression: lines containing multi-byte codepoints (e.g. the
    // `─` box-drawing char used in section headers throughout the
    // preset) used to be clipped at a byte boundary, producing
    // invalid UTF-8 that crashed `frame:text` with a misleading
    // "expected string or number, got nil" via mlua's coerce_string.
    // paint.buffer must clip at CHAR boundaries.
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        -- One line, 80 `─` chars (240 bytes). area.w = 40 — must clip
        -- to 40 chars (= 120 bytes), not 40 bytes.
        buf:apply({ gauchito.splice.new(0, 0, string.rep("─", 80)) })

        local view = gauchito.view.new(buf)
        gauchito.frame:clear()
        gauchito.paint.view(view, {x=0, y=0, w=40, h=1}, {
            scroll_row = 0,
            show_cursor = false,
            show_selection = false,
        })
        -- No assertion needed — if we reached here, no UTF-8 boundary
        -- bug. Previously this raised:
        --   "error converting Lua nil to String (expected string or number)"
    "#,
    )
    .unwrap();
}

#[test]
fn paint_buffer_renders_835_line_file_without_panic() {
    // Mirrors what happens when the distro opens vim.lua (~835 lines)
    // and the first redraw fires. The freeze report points here; if
    // there's a runaway loop or an mlua error in run-coalescing, this
    // surfaces it.
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        local lines = {}
        for i = 1, 835 do
            lines[#lines + 1] = string.format("-- line %d with some content", i)
        end
        local text = table.concat(lines, "\n") .. "\n"
        buf:apply({ gauchito.splice.new(0, 0, text) })

        local view = gauchito.view.new(buf)
        gauchito.frame:clear()
        -- Render a moderately tall window with selection synthesis.
        gauchito.paint.view(view, {x=0, y=0, w=120, h=50}, {
            scroll_row = 0,
            show_cursor = true,
            show_selection = true,
        })
    "#,
    )
    .unwrap();
}

#[test]
fn paint_buffer_renders_trailing_newline_correctly() {
    // A file ending with `\n` creates a navigable empty last line —
    // vim-like. Lock the contract: line_count, last line's content,
    // and EOF cursor positioning all account for the trailing line.
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        buf:apply({ gauchito.splice.new(0, 0, "hello\nworld\n") })
        assert(buf:line_count() == 3,
            'file ending in \\n should have an extra empty last line; got ' ..
            tostring(buf:line_count()))
        assert(buf:line(0) == "hello")
        assert(buf:line(1) == "world")
        assert(buf:line(2) == "", 'trailing line is empty')
        -- bol of the trailing empty line is text.len_chars() (12).
        assert(buf:bol(2) == 12)
        assert(buf:eol(2) == 12)

        -- doc_end returns text.len_chars() — past the trailing `\n`.
        local end_pos = buf:doc_end()
        assert(end_pos == 12, 'doc_end should equal len_chars; got ' .. tostring(end_pos))
        -- line_at on EOF lands on the trailing empty line, not on the
        -- previous "world" line.
        assert(buf:line_at(end_pos) == 2,
            'line_at(EOF) for file ending in \\n must be the trailing empty line; got ' ..
            tostring(buf:line_at(end_pos)))
    "#,
    )
    .unwrap();
}

#[test]
fn paint_buffer_renders_no_trailing_newline_correctly() {
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        buf:apply({ gauchito.splice.new(0, 0, "hello\nworld") })
        assert(buf:line_count() == 2, 'no trailing newline → 2 lines; got ' ..
            tostring(buf:line_count()))
        assert(buf:line(1) == "world")

        local end_pos = buf:doc_end()
        assert(end_pos == 11)
        -- line_at(EOF) clamps onto the last char's line ("world").
        assert(buf:line_at(end_pos) == 1)
    "#,
    )
    .unwrap();
}

#[test]
fn cursor_at_eof_after_trailing_newline_paints_on_empty_row() {
    // Reproduces the user-reported bug: after pressing G on a file
    // ending in \n, the cursor should land on the empty row below,
    // not "after the last char of the last line."
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        buf:apply({ gauchito.splice.new(0, 0, "hello\nworld\n") })

        -- Simulate `G` to doc_end, then resolve the row paint.cursor
        -- would use.
        local view = gauchito.view.new(buf)
        local end_pos = buf:doc_end()
        view:set_selection(gauchito.selection.new({{anchor=end_pos, head=end_pos}}))

        local r = view.selection:primary_range()
        local line = buf:line_at(r.head)
        local col = r.head - buf:bol(line)

        -- Row 2 (the empty trailing line), col 0.
        assert(line == 2, 'cursor must land on the trailing empty line; got line=' ..
            tostring(line))
        assert(col == 0, 'cursor must be at col 0 on the new empty line; got col=' ..
            tostring(col))
    "#,
    )
    .unwrap();
}

#[test]
fn paint_buffer_handles_motions_on_long_buffer_without_runaway() {
    // Exercises w/e/b/W/E/B/ge/gE-style position math on a buffer
    // with many whitespace runs (mimics the layout of indented lua).
    // If any motion under the substrate has an unguarded `while`, this
    // freezes the test rather than passing.
    let rt = make();
    rt.eval::<()>(
        r#"
        local buf = gauchito.buf.scratch()
        local text = ""
        for i = 1, 200 do
            text = text .. "    local x = foo(bar, baz)  -- comment\n"
        end
        buf:apply({ gauchito.splice.new(0, 0, text) })

        local len = buf:len()
        local p = 0
        -- Walk forward by word until we hit EOF.
        local steps = 0
        while p < len and steps < 100000 do
            local q = buf:scan_class_fwd(p)
            if q == p then p = p + 1 else p = q end
            steps = steps + 1
        end
        assert(p >= len, 'scan_class_fwd should walk to EOF; got p=' .. tostring(p))

        -- Walk backward.
        p = len
        steps = 0
        while p > 0 and steps < 100000 do
            local q = buf:scan_class_bwd(p)
            if q == p then p = p - 1 else p = q end
            steps = steps + 1
        end
        assert(p == 0, 'scan_class_bwd should walk to BOF; got p=' .. tostring(p))
    "#,
    )
    .unwrap();
}
