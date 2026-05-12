-- Vim preset for gauchito.
--
-- Three modes (normal/visual/insert), counts, operator-pending (d), char-find
-- (f/F/t/T), prefix sequences (gg/ge, ctrl-w-*), big-word motions, paragraph
-- motions, bracket match, multi-cursor (C/,), undo/redo.
--
-- Algebra and operator-pending live in `bv.*` (prelude). Here we just declare
-- motion tables and wire keys.

local k = bv.k

-- ── Motion tables ──────────────────────────────────────────────────────────

local collapse_motions = {
    h     = bv.collapse(k.move_left_inline),
    l     = bv.collapse(k.move_right_inline),
    j     = bv.collapse(k.move_down),
    k     = bv.collapse(k.move_up),
    w     = bv.collapse(k.move_word_forward),
    b     = bv.collapse(k.move_word_backward),
    e     = bv.collapse(k.move_word_end),
    W     = bv.collapse(k.move_word_forward_big),
    B     = bv.collapse(k.move_word_backward_big),
    E     = bv.collapse(k.move_word_end_big),
    ["0"] = bv.collapse(k.move_line_start),
    ["$"] = bv.collapse(k.move_line_end),
    ["^"] = bv.collapse(k.move_first_non_whitespace),
    G     = bv.collapse(k.move_doc_end),
    ["{"] = bv.collapse(k.move_paragraph_backward),
    ["}"] = bv.collapse(k.move_paragraph_forward),
    ["%"] = bv.collapse(k.match_bracket),
}

local extend_motions = {
    h     = bv.extend(k.move_left_inline),
    l     = bv.extend(k.move_right_inline),
    j     = bv.extend(k.move_down),
    k     = bv.extend(k.move_up),
    w     = bv.extend(k.move_word_forward),
    b     = bv.extend(k.move_word_backward),
    e     = bv.extend(k.move_word_end),
    W     = bv.extend(k.move_word_forward_big),
    B     = bv.extend(k.move_word_backward_big),
    E     = bv.extend(k.move_word_end_big),
    ["0"] = bv.extend(k.move_line_start),
    ["$"] = bv.extend(k.move_line_end),
    ["^"] = bv.extend(k.move_first_non_whitespace),
    G     = bv.extend(k.move_doc_end),
    ["{"] = bv.extend(k.move_paragraph_backward),
    ["}"] = bv.extend(k.move_paragraph_forward),
    ["%"] = bv.extend(k.match_bracket),
}

-- Raw kernels for operator-pending range computation.
-- `inclusive` motions cover the destination char in the operation.
local motion_kernels = {
    h     = { fn = k.move_left, inclusive = true },
    l     = { fn = k.move_right, inclusive = true },
    j     = { fn = k.move_down, inclusive = false, linewise = true },
    k     = { fn = k.move_up,   inclusive = false, linewise = true },
    w     = { fn = k.move_word_forward, inclusive = false },
    b     = { fn = k.move_word_backward, inclusive = false },
    e     = { fn = k.move_word_end, inclusive = true },
    W     = { fn = k.move_word_forward_big, inclusive = false },
    B     = { fn = k.move_word_backward_big, inclusive = false },
    E     = { fn = k.move_word_end_big, inclusive = true },
    ["0"] = { fn = k.move_line_start, inclusive = false },
    ["$"] = { fn = k.move_line_end, inclusive = true },
    ["^"] = { fn = k.move_first_non_whitespace, inclusive = false },
    G     = { fn = k.move_doc_end, inclusive = false },
    ["{"] = { fn = k.move_paragraph_backward, inclusive = false },
    ["}"] = { fn = k.move_paragraph_forward, inclusive = false },
    ["%"] = { fn = k.match_bracket, inclusive = true },
}

local char_kernels = {
    f = k.find_char_forward,
    F = k.find_char_backward,
    t = k.find_char_forward_before,
    T = k.find_char_backward_after,
}

-- ── Mode helpers ───────────────────────────────────────────────────────────

local function enter_insert(ctx)
    ctx:transaction_start()
    ctx:set_mode("insert")
    ctx:set_cursor_style("bar")
end

local function enter_normal(ctx)
    ctx:set_mode("normal")
    ctx:set_cursor_style("block")
end

local function enter_visual(ctx)
    ctx:set_mode("visual")
    ctx:set_cursor_style("block")
end

-- ── Operator d ─────────────────────────────────────────────────────────────

local delete_line = bv.seq(
    bv.lift(k.select_whole_line),
    bv.fold(bv.delete_selection)
)

local op_d = bv.operator({
    mutation    = bv.fold(bv.delete_selection),
    motions     = motion_kernels,
    char_finds  = char_kernels,
    self_key    = "d",
    self_action = delete_line,
})

-- ── Standalone char-find (no operator) ─────────────────────────────────────

local function char_find(flavour, ck, count)
    return function(ctx)
        local ctx, ch = bv.read_char()
        if ch == nil then return end
        local buf = ctx:text()
        ctx:map_selections(function(anchor, head)
            local h = head
            for _ = 1, count do h = ck(buf, h, ch) end
            if flavour == "extend" then return anchor, h end
            return h, h
        end)
    end
end

-- ── Prefix sequences ───────────────────────────────────────────────────────

local function g_prefix(motions)
    return function(ctx)
        local ctx, key = bv.read_key()
        if key == "g" then
            motions.doc_start(ctx)
        elseif key == "e" then
            motions.doc_end(ctx)
        end
    end
end

local g_collapse = {
    doc_start = bv.collapse(k.move_doc_start),
    doc_end   = bv.collapse(k.move_doc_end),
}
local g_extend = {
    doc_start = bv.extend(k.move_doc_start),
    doc_end   = bv.extend(k.move_doc_end),
}

local function ctrl_w_prefix(ctx)
    local ctx, key = bv.read_key()
    if key == "v" then
        ctx:split_vertical()
    elseif key == "s" then
        ctx:split_horizontal()
    elseif key == "w" then
        ctx:focus_next()
    elseif key == "W" then
        ctx:focus_prev()
    elseif key == "q" then
        ctx:close_view()
    end
end

-- ── Counted dispatch ───────────────────────────────────────────────────────

local function dispatch_counted(ctx, count, key)
    local n = (count == 0) and 1 or count

    if key == "d" then
        op_d(ctx, n); return
    end
    if key == "g" then
        g_prefix(g_collapse)(ctx); return
    end

    local ck = char_kernels[key]
    if ck then
        char_find("collapse", ck, n)(ctx); return
    end

    local m = collapse_motions[key]
    if m then
        bv.rep(n, m)(ctx); return
    end

    if key == "x" then
        bv.rep(n, bv.fold(bv.delete_char_forward))(ctx); return
    end
    if key == "X" then bv.rep(n, bv.fold(bv.delete_char_backward))(ctx) end
end

local function vdispatch_counted(ctx, count, key)
    local n = (count == 0) and 1 or count

    if key == "g" then
        g_prefix(g_extend)(ctx); return
    end

    local ck = char_kernels[key]
    if ck then
        char_find("extend", ck, n)(ctx); return
    end

    local m = extend_motions[key]
    if m then bv.rep(n, m)(ctx) end
end

-- ── Mode keymaps ───────────────────────────────────────────────────────────

local normal_keys = {
    -- motion (count-1 fast paths; counts go through __fallback)
    h          = collapse_motions.h,
    l          = collapse_motions.l,
    j          = collapse_motions.j,
    k          = collapse_motions.k,
    w          = collapse_motions.w,
    b          = collapse_motions.b,
    e          = collapse_motions.e,
    W          = collapse_motions.W,
    B          = collapse_motions.B,
    E          = collapse_motions.E,
    ["0"]      = collapse_motions["0"],
    ["$"]      = collapse_motions["$"],
    ["^"]      = collapse_motions["^"],
    G          = collapse_motions.G,
    ["{"]      = collapse_motions["{"],
    ["}"]      = collapse_motions["}"],
    ["%"]      = collapse_motions["%"],

    -- char-find
    f          = char_find("collapse", k.find_char_forward, 1),
    F          = char_find("collapse", k.find_char_backward, 1),
    t          = char_find("collapse", k.find_char_forward_before, 1),
    T          = char_find("collapse", k.find_char_backward_after, 1),

    -- prefixes
    g          = g_prefix(g_collapse),
    ["ctrl-w"] = ctrl_w_prefix,

    -- operator
    d          = function(ctx) op_d(ctx, 1) end,
    D          = bv.seq(
        bv.extend(k.move_line_end),
        bv.expand_high(1),
        bv.fold(bv.delete_selection)
    ),

    -- single-key edits
    x          = bv.fold(bv.delete_char_forward),
    X          = bv.fold(bv.delete_char_backward),
    J          = bv.seq(
        bv.collapse(k.move_line_end),
        bv.collapse(k.move_right),
        bv.fold(bv.delete_char_backward),
        function(ctx) ctx:edit(bv.insert_text(ctx:text(), ctx:selection(), " ")) end
    ),

    -- multi-cursor
    C          = bv.add_cursor(k.move_down),
    [","]      = bv.keep_primary,

    -- history / system
    u          = function(ctx) ctx:undo() end,
    ["ctrl-r"] = function(ctx) ctx:redo() end,
    ["ctrl-s"] = function(ctx) ctx:save() end,
    ["ctrl-q"] = function(ctx) ctx:quit() end,

    -- mode switches
    i          = enter_insert,
    I          = bv.seq(bv.collapse(k.move_first_non_whitespace), enter_insert),
    a          = bv.seq(bv.collapse(k.move_right), enter_insert),
    A          = bv.seq(bv.collapse(k.move_line_end), bv.collapse(k.move_right), enter_insert),

    o          = bv.seq(
        bv.collapse(k.move_line_end),
        bv.collapse(k.move_right),
        enter_insert,
        function(ctx) ctx:edit(bv.insert_char(ctx:text(), ctx:selection(), "\n")) end
    ),
    O          = bv.seq(
        bv.collapse(k.move_line_start),
        enter_insert,
        function(ctx) ctx:edit(bv.insert_char(ctx:text(), ctx:selection(), "\n")) end,
        bv.collapse(k.move_left)
    ),

    v          = enter_visual,

    -- counts: any leading digit kicks off a count read.
    __fallback = function(ctx, key)
        if not bv.is_digit(key) then return end
        local ctx, count, next_key = bv.read_count(tonumber(key))
        dispatch_counted(ctx, count, next_key)
    end,
}

-- Visual-mode "make selection inclusive on delete": same shape as
-- bv.expand_high(1). Reused for v-mode `d` and `x`.
local visual_delete = bv.seq(
    bv.expand_high(1),
    bv.fold(bv.delete_selection),
    enter_normal
)

local visual_keys = {
    h          = extend_motions.h,
    l          = extend_motions.l,
    j          = extend_motions.j,
    k          = extend_motions.k,
    w          = extend_motions.w,
    b          = extend_motions.b,
    e          = extend_motions.e,
    W          = extend_motions.W,
    B          = extend_motions.B,
    E          = extend_motions.E,
    ["0"]      = extend_motions["0"],
    ["$"]      = extend_motions["$"],
    ["^"]      = extend_motions["^"],
    G          = extend_motions.G,
    ["{"]      = extend_motions["{"],
    ["}"]      = extend_motions["}"],
    ["%"]      = extend_motions["%"],

    f          = char_find("extend", k.find_char_forward, 1),
    F          = char_find("extend", k.find_char_backward, 1),
    t          = char_find("extend", k.find_char_forward_before, 1),
    T          = char_find("extend", k.find_char_backward_after, 1),

    g          = g_prefix(g_extend),

    o          = function(ctx)
        ctx:map_selections(function(anchor, head) return head, anchor end)
    end,
    d          = visual_delete,
    x          = visual_delete,
    esc        = bv.seq(
        function(ctx)
            ctx:map_selections(function(_, head) return head, head end)
        end,
        enter_normal
    ),

    ["ctrl-s"] = function(ctx) ctx:save() end,
    ["ctrl-q"] = function(ctx) ctx:quit() end,

    __fallback = function(ctx, key)
        if not bv.is_digit(key) then return end
        local ctx, count, next_key = bv.read_count(tonumber(key))
        vdispatch_counted(ctx, count, next_key)
    end,
}

local insert_keys = {
    esc        = bv.seq(
        bv.collapse(k.move_left_inline),
        function(ctx) ctx:transaction_commit() end,
        enter_normal
    ),

    left       = bv.collapse(k.move_left),
    right      = bv.collapse(k.move_right),
    up         = bv.collapse(k.move_up),
    down       = bv.collapse(k.move_down),
    backspace  = bv.fold(bv.delete_char_backward),
    del        = bv.fold(bv.delete_char_forward),
    enter      = bv.fold(bv.insert_newline),
    tab        = function(ctx) ctx:edit(bv.insert_text(ctx:text(), ctx:selection(), "    ")) end,
    home       = bv.collapse(k.move_line_start),
    ["end"]    = bv.collapse(k.move_line_end),
    ["ctrl-s"] = function(ctx) ctx:save() end,
    ["ctrl-q"] = function(ctx) ctx:quit() end,

    -- Printable fall-through. `ch` is nil for non-printable keys.
    __fallback = function(ctx, _, ch)
        if ch then
            ctx:edit(bv.insert_char(ctx:text(), ctx:selection(), ch))
        end
    end,
}

return {
    initial_mode = "normal",
    modes = {
        normal = { keys = normal_keys },
        visual = { keys = visual_keys },
        insert = { keys = insert_keys },
    },
}
