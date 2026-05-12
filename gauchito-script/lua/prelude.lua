-- Gauchito Lua prelude.
--
-- Loaded once at startup, before user config. Wraps the kernel table `bv`
-- (registered by Rust) with composition combinators and yield helpers.
--
-- Naming:
--   bv.k.*               per-cursor motion / selection / char-find kernels
--   bv.delete_*          mutation kernels (return changesets)
--   bv.insert_*          mutation kernels (return changesets)
--
--   bv.collapse(kernel)  ctx-action: move head, collapse anchor onto it
--   bv.extend(kernel)    ctx-action: move head, anchor stays
--   bv.select(kernel)    ctx-action: head moves; old head becomes anchor
--   bv.lift(kernel)      ctx-action: kernel returns {anchor, head}
--   bv.fold(mutation)    ctx-action: apply mutation across selections
--   bv.rep(n, op)        ctx-action: repeat `op` `n` times
--   bv.seq(...)          ctx-action: run ops in order
--
--   bv.read_key()        yield, return (ctx, key)
--   bv.read_char()       yield, return (ctx, ch)  -- ch nil if non-printable
--   bv.read_count(d)     yield until non-digit; return (ctx, count, key)
--   bv.is_digit(key)     true if `key` is a single digit
--
--   bv.expand_high(n)    grow each range's high end by n
--   bv.add_cursor(k)     push a new cursor at kernel(buf, primary.head)
--   bv.keep_primary      drop every secondary cursor
--   bv.operator(opts)    operator-pending combinator (see below)

-- ── Yield helpers ──────────────────────────────────────────────────────────

function bv.read_key()
    local ctx, key = coroutine.yield()
    return ctx, key
end

function bv.read_char()
    local ctx, _, ch = coroutine.yield()
    return ctx, ch
end

-- ── Digits / counts ───────────────────────────────────────────────────────

function bv.is_digit(key)
    if type(key) ~= "string" or #key ~= 1 then return false end
    local n = tonumber(key)
    return n ~= nil and n >= 0 and n <= 9
end

-- Read keys, accumulating digits into `count`. Stops on the first non-digit
-- and returns it. `initial` seeds the count (use 0 if no digit was consumed
-- yet, or the leading digit's value if dispatch entered via `__fallback`).
-- Returns `(ctx, count, first_non_digit_key)`. `count == 0` means none seen.
function bv.read_count(initial)
    local count = initial or 0
    while true do
        local ctx, key = bv.read_key()
        if not bv.is_digit(key) then return ctx, count, key end
        count = count * 10 + tonumber(key)
    end
end

-- ── Motion combinators ─────────────────────────────────────────────────────
-- Each takes a per-cursor kernel `(buf, head) -> head` and returns a
-- ctx-action. The action calls `ctx:map_selections(fn)` so all anchors are
-- updated atomically.

function bv.collapse(kernel)
    return function(ctx)
        local buf = ctx:text()
        ctx:map_selections(function(_, head)
            local h = kernel(buf, head)
            return h, h
        end)
    end
end

function bv.extend(kernel)
    return function(ctx)
        local buf = ctx:text()
        ctx:map_selections(function(anchor, head)
            return anchor, kernel(buf, head)
        end)
    end
end

function bv.select(kernel)
    return function(ctx)
        local buf = ctx:text()
        ctx:map_selections(function(_, head)
            return head, kernel(buf, head)
        end)
    end
end

-- lift: kernel returns a {anchor, head} table (selection-shape kernels).
function bv.lift(kernel)
    return function(ctx)
        local buf = ctx:text()
        ctx:map_selections(function(anchor, head)
            local r = kernel(buf, anchor, head)
            return r.anchor, r.head
        end)
    end
end

-- ── Mutation combinator ────────────────────────────────────────────────────

function bv.fold(mutation)
    return function(ctx)
        ctx:edit(mutation(ctx:text(), ctx:selection()))
    end
end

-- ── Sequencing ─────────────────────────────────────────────────────────────

function bv.rep(n, op)
    return function(ctx)
        for _ = 1, n do op(ctx) end
    end
end

function bv.seq(...)
    local ops = { ... }
    return function(ctx)
        for _, op in ipairs(ops) do op(ctx) end
    end
end

-- ── Selection-shape combinators ────────────────────────────────────────────

-- Grow each range's high end by `n` chars. Forward ranges grow head;
-- backward ranges grow anchor; collapsed ranges grow head. Used to make
-- exclusive motions inclusive (operator-pending) and to make visual-mode
-- selections character-inclusive at delete time.
function bv.expand_high(n)
    return function(ctx)
        ctx:map_selections(function(anchor, head)
            if head >= anchor then return anchor, head + n end
            return anchor + n, head
        end)
    end
end

-- ── Multi-cursor combinators ───────────────────────────────────────────────

-- Push a new collapsed cursor at `kernel(buf, primary.head)`. No-op if the
-- kernel doesn't advance (so adding a cursor below the last line is silent).
function bv.add_cursor(kernel)
    return function(ctx)
        local primary = ctx:selection():primary()
        local new = kernel(ctx:text(), primary.head)
        if new ~= primary.head then
            ctx:push_cursor(new, new)
        end
    end
end

-- Drop every secondary cursor, keeping just the primary range collapsed.
function bv.keep_primary(ctx)
    local sel = ctx:selection()
    local primary_head = sel:primary().head
    -- Remove non-primary ranges from highest index down.
    local n = sel:len()
    local primary_idx = sel:primary_idx()
    for i = n - 1, 0, -1 do
        if i ~= primary_idx then ctx:remove_cursor(i) end
    end
    ctx:map_selections(function(_, _) return primary_head, primary_head end)
end

-- ── Operator-pending combinator ────────────────────────────────────────────
--
-- Build an operator (d, c, y, …) on top of `bv.read_count` + `bv.read_key`.
-- Yields once for the post-count + motion key; if the motion is char-find
-- (f/F/t/T) yields again for the char.
--
-- opts = {
--     mutation    = ctx-action applied after the range is computed,
--                   e.g. bv.fold(bv.delete_selection)
--     motions     = { [key] = { fn = kernel,
--                               inclusive = bool,    -- include destination char
--                               linewise  = bool },  -- expand to whole lines
--                     … }
--     char_finds  = { [key] = char_kernel, … }       (always inclusive)
--     self_key    = optional, e.g. "d" — triggers self_action when key matches
--     self_action = optional ctx-action repeated `count_pre` times on self_key
-- }
--
-- The returned function is `op(ctx, count_pre)`. count_pre defaults to 1.
function bv.operator(opts)
    return function(ctx, count_pre)
        count_pre = count_pre or 1
        local ctx, count_post, key = bv.read_count(0)
        local total = (count_post == 0) and count_pre or (count_pre * count_post)

        -- Self-key shortcut (dd, yy, cc).
        if opts.self_key and key == opts.self_key and opts.self_action then
            bv.rep(count_pre, opts.self_action)(ctx)
            return
        end

        -- Char-find motions — always inclusive.
        local ck = opts.char_finds and opts.char_finds[key]
        if ck then
            local ctx, ch = bv.read_char()
            if ch == nil then return end
            local buf = ctx:text()
            ctx:map_selections(function(_, head)
                local h = head
                for _ = 1, total do h = ck(buf, h, ch) end
                return math.min(head, h), math.max(head, h) + 1
            end)
            opts.mutation(ctx)
            return
        end

        -- Regular motion.
        local mk = opts.motions and opts.motions[key]
        if mk == nil then return end
        local buf = ctx:text()
        ctx:map_selections(function(_, head)
            local h = head
            for _ = 1, total do h = mk.fn(buf, h) end
            local from, to = math.min(head, h), math.max(head, h)

            -- Linewise (j/k under operator): expand to whole-line boundaries.
            -- vim's `dj` deletes two lines, not just to col-K of the next line.
            if mk.linewise then
                local from_line = bv.k.select_whole_line(buf, 0, from)
                local to_line   = bv.k.select_whole_line(buf, 0, to)
                return from_line.anchor, to_line.head
            end

            if mk.inclusive then to = to + 1 end
            if to > from then return from, to end
            return head, head
        end)
        opts.mutation(ctx)
    end
end
