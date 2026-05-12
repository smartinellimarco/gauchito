-- Micro preset for gauchito.
--
-- Single-mode editor, readline-style. Arrow keys move, printable chars insert
-- via the `__fallback` handler, ctrl-combos for save/quit/undo/redo.

local k = bv.k

local keys = {
    -- Motion.
    left      = bv.collapse(k.move_left),
    right     = bv.collapse(k.move_right),
    up        = bv.collapse(k.move_up),
    down      = bv.collapse(k.move_down),
    home      = bv.collapse(k.move_line_start),
    ["end"]   = bv.collapse(k.move_line_end),

    -- Editing.
    backspace = bv.fold(bv.delete_char_backward),
    del       = bv.fold(bv.delete_char_forward),
    enter     = bv.fold(bv.insert_newline),
    tab       = function(ctx) ctx:edit(bv.insert_text(ctx:text(), ctx:selection(), "    ")) end,

    -- Commands.
    ["ctrl-s"] = function(ctx) ctx:save() end,
    ["ctrl-q"] = function(ctx) ctx:quit() end,
    ["ctrl-z"] = function(ctx) ctx:undo() end,
    ["ctrl-y"] = function(ctx) ctx:redo() end,

    -- Printable fall-through.
    __fallback = function(ctx, _, ch)
        if ch then
            ctx:edit(bv.insert_char(ctx:text(), ctx:selection(), ch))
        end
    end,
}

return {
    initial_mode = "edit",
    modes = {
        edit = { keys = keys },
    },
}
