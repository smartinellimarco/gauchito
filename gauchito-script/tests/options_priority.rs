//! Option-merge priority through `buf.open`.
//!
//! The substrate's contract: `sniffed < user_opts`.
//! Each layer is a sparse `PartialBufferOptions` table; missing
//! fields fall through to the next layer.
//!
//! We exercise this purely at the Lua level because the contract is
//! defined by `gauchito.options.merge` (left-to-right "later wins")
//! and the call site in `gauchito.buf.open`.

use gauchito_script::ScriptRuntime;

fn make() -> ScriptRuntime {
    ScriptRuntime::new(vec![]).unwrap().0
}

#[test]
fn merge_later_layers_win() {
    let rt = make();
    rt.eval::<()>(
        r#"
        local sniffed = { line_ending = 'crlf', final_newline = true,  bom = false }
        local mid     = { line_ending = 'lf',   trim_trailing_whitespace = true }
        local user    = { line_ending = 'lf',   indent = { unit = '  ', tab_width = 2 } }

        -- Call order = priority order, lowest-first. Later layers
        -- overwrite earlier set fields; unset fields fall through.
        local merged = gauchito.options.merge(sniffed, mid, user)

        -- All three layers set line_ending; user wins.
        assert(merged.line_ending == 'lf', 'user wins line_ending; got ' .. tostring(merged.line_ending))
        -- final_newline only set by sniffed → passes through.
        assert(merged.final_newline == true)
        -- trim_trailing_whitespace only by mid → passes through.
        assert(merged.trim_trailing_whitespace == true)
        -- indent only by user.
        assert(merged.indent.unit == '  ')
        assert(merged.indent.tab_width == 2)
        -- bom only by sniffed.
        assert(merged.bom == false)
    "#,
    )
    .unwrap();
}

#[test]
fn merge_ignores_extra_keys() {
    // Lua's options.merge whitelists OPTION_KEYS — extra keys in any
    // layer drop on the floor (no leak into the resolved options).
    let rt = make();
    rt.eval::<()>(
        r#"
        local merged = gauchito.options.merge(
            { bogus = 42, line_ending = 'lf' },
            { also_bogus = 'x' }
        )
        assert(merged.bogus == nil, 'unknown keys must not leak through merge')
        assert(merged.also_bogus == nil)
        assert(merged.line_ending == 'lf')
    "#,
    )
    .unwrap();
}

#[test]
fn resolve_fills_substrate_defaults_for_unset_fields() {
    let rt = make();
    rt.eval::<()>(
        r#"
        -- Empty partial: every field comes from SUBSTRATE_DEFAULTS.
        local resolved = gauchito.options.resolve({})
        assert(resolved.line_ending == 'lf')
        assert(resolved.final_newline == true)
        assert(resolved.bom == false)
        assert(resolved.trim_trailing_whitespace == true)
        assert(resolved.indent.unit == '    ')
        assert(resolved.indent.tab_width == 4)

        -- Partial overrides survive resolve.
        local r2 = gauchito.options.resolve({ line_ending = 'crlf' })
        assert(r2.line_ending == 'crlf')
        assert(r2.final_newline == true, 'unset field falls through to default')
    "#,
    )
    .unwrap();
}

#[test]
fn buf_open_merges_sniffed_then_user() {
    // Stub `gauchito.io.read` so we can drive the buf.open priority test without hitting disk.
    let rt = make();
    rt.eval::<()>(
        r#"
        local seen_path
        local original_read = gauchito.io.read

        -- Fake io.read: returns an empty rope + a sniffed partial.
        gauchito.io.read = function(path)
            seen_path = path
            return setmetatable({
                await = function(_)
                    return {
                        rope = nil,
                        sniffed = { line_ending = 'crlf', final_newline = true },
                    }
                end,
            }, {})
        end
        -- Patch buf.from_rope to capture the merged opts.
        local captured_opts
        local original_from_rope = gauchito.buf.from_rope
        gauchito.buf.from_rope = function(rope, path, opts)
            captured_opts = opts
            return {}
        end

        -- User opts win over sniffed.
        local _ = gauchito.buf.open('/fake/path', {
            line_ending = 'cr',   -- nonsense value, only checking pass-through
            indent = { unit = '\t', tab_width = 8 },
        }):await()

        -- Restore originals so other tests aren't affected.
        gauchito.io.read = original_read
        gauchito.buf.from_rope = original_from_rope

        assert(captured_opts.line_ending == 'cr',
            'user opt wins; got ' .. tostring(captured_opts.line_ending))
        assert(captured_opts.final_newline == true,
            'sniffed field passes through when no higher layer sets it')
        assert(captured_opts.indent.tab_width == 8,
            'user-only field present')
    "#,
    )
    .unwrap();
}
