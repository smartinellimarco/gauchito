--!strict
-- Gauchito Lua prelude — loaded once at startup, before the distro config.
--
-- Installs the pure-Lua substrate tables (event bus, options, tasks, keys,
-- io, paint, viewport) on top of the `gauchito` global that Rust creates.
-- The Rust-injected parts (gauchito.buf/view/selection/splice,
-- gauchito.frame, and the Buffer/View/Selection/... handles) are declared
-- in `api/gauchito.d.luau`; the types referenced below come from there.

-- ── Event bus ─────────────────────────────────────────────────────────────

gauchito.event = {
    _next_id = 1,
    _subs    = {},
}

local Handle = {}
Handle.__index = Handle

function Handle:cancel()
    local subs = gauchito.event._subs[self.name]
    if subs then subs[self.id] = nil end
end

local function subscribe(name: string, fn: (any) -> (), once: boolean): Handle
    if type(name) ~= 'string' then
        error('gauchito.event.on: name must be a string')
    end
    if type(fn) ~= 'function' then
        error('gauchito.event.on: callback must be a function')
    end
    local id = gauchito.event._next_id
    gauchito.event._next_id = id + 1
    local subs = gauchito.event._subs[name]
    if not subs then
        subs = {}
        gauchito.event._subs[name] = subs
    end
    subs[id] = { fn = fn, once = once }
    return setmetatable({ name = name, id = id }, Handle)
end

function gauchito.event.on(name: string, fn: (any) -> ()): Handle return subscribe(name, fn, false) end

function gauchito.event.once(name: string, fn: (any) -> ()): Handle return subscribe(name, fn, true) end

function gauchito.event.emit(name: string, payload: any)
    local subs = gauchito.event._subs[name]
    if not subs then return end
    local ids = {}
    for id in pairs(subs) do ids[#ids + 1] = id end
    table.sort(ids)
    for _, id in ipairs(ids) do
        local sub = subs[id]
        if sub then
            if sub.once then subs[id] = nil end
            sub.fn(payload)
        end
    end
end

-- ── Document options ──────────────────────────────────────────────────────

gauchito.options = {}

local OPTION_KEYS = {
    line_ending              = true,
    final_newline            = true,
    bom                      = true,
    trim_trailing_whitespace = true,
    indent                   = true,
}

function gauchito.options.merge(...: PartialBufferOptions): PartialBufferOptions
    local out = {}
    for i = 1, select('#', ...) do
        local layer = select(i, ...) or {}
        for k in pairs(OPTION_KEYS) do
            if layer[k] ~= nil then out[k] = layer[k] end
        end
    end
    return out
end

local SUBSTRATE_DEFAULTS = {
    line_ending              = 'lf',
    final_newline            = true,
    bom                      = false,
    trim_trailing_whitespace = true,
    indent                   = { unit = '    ', tab_width = 4 },
}

function gauchito.options.resolve(partial: PartialBufferOptions?): PartialBufferOptions
    return gauchito.options.merge(SUBSTRATE_DEFAULTS, partial or {})
end

-- ── Tasks ─────────────────────────────────────────────────────────────────

gauchito.task = {}
gauchito.task.spawn = gauchito.__task_spawn

local Awaitable = {}
Awaitable.__index = Awaitable

function Awaitable:await()
    local value, err = coroutine.yield(self)
    if err ~= nil then error(err) end
    return value
end

function gauchito.task.sleep(ms: number): Awaitable
    return setmetatable({ _kind = 'sleep', _ms = ms }, Awaitable)
end

-- ── Keys ──────────────────────────────────────────────────────────────────
--
-- The substrate hands Lua a structured event per terminal key:
--
--     { code, ch, ctrl, alt, shift }
--
-- Naming convention ('ctrl-a', '<C-a>', …) is policy and lives here, not
-- in Rust. `gauchito.keys.name(event)` produces the vim-flavored name —
-- distros override it freely.
--
-- `gauchito.keys.read():await()` resumes with `(name, ch, event)`:
--   • `name`  — string from `gauchito.keys.name(event)`
--   • `ch`    — printable char if applicable, else nil
--   • `event` — the raw `{code, ch, ctrl, alt, shift}` table
--
-- Distros that want a different naming scheme replace `gauchito.keys.name`
-- before spawning the read loop. Inside a chord handler the same call
-- yields and resumes on the next key — chord trees are coroutines.

gauchito.keys = {}

-- Vim-flavored namer: 'h', 'enter', 'ctrl-c', 'alt-x', 'ctrl-alt-del'.
-- Distros wanting `<C-h>` style replace this function wholesale.
function gauchito.keys.name(event: KeyEvent): string
    local base
    if event.code == 'char' or event.code == 'space' then
        base = event.ch or event.code
    else
        base = event.code
    end
    if event.ctrl and event.alt then return 'ctrl-alt-' .. base end
    if event.ctrl then return 'ctrl-' .. base end
    if event.alt then return 'alt-' .. base end
    return base
end

-- Printable iff a bare character with no modifier that prevents echo.
local function event_ch(event: KeyEvent): string?
    if event.code ~= 'char' and event.code ~= 'space' then return nil end
    if event.ctrl or event.alt then return nil end
    return event.ch
end

local KeysAwaitable = {}
KeysAwaitable.__index = KeysAwaitable

function KeysAwaitable:await()
    local event = coroutine.yield(self)
    return gauchito.keys.name(event), event_ch(event), event
end

function gauchito.keys.read(): KeysAwaitable
    return setmetatable({ _kind = 'keys.read' }, KeysAwaitable)
end

-- ── I/O ───────────────────────────────────────────────────────────────────

gauchito.io = gauchito.io or {}

function gauchito.io.read(path: string): Awaitable
    return setmetatable({ _kind = 'io.read', _path = path }, Awaitable)
end

-- ── Buffer loading convenience ───────────────────────────────────────────

gauchito.buf = gauchito.buf or {}

-- Option-merge priority for buf.open (lowest → highest):
--   1. sniffed         (what's currently on disk; only sets fields we
--                        could detect — line_ending, final_newline, bom)
--   2. user `opts`     (caller-provided overrides)
-- `gauchito.options.merge` is left-to-right "later wins", so the call
-- order below reads bottom-up by priority.
function gauchito.buf.open(path: string, opts: PartialBufferOptions?): Awaitable
    return setmetatable({
        _kind = 'buf.open',
        _path = path,
        _opts = opts,
        await = function(self)
            local read_res = gauchito.io.read(self._path):await()
            local merged = gauchito.options.merge(read_res.sniffed, self._opts or {})
            return gauchito.buf.from_rope(read_res.rope, self._path, merged)
        end,
    }, { __index = Awaitable })
end

-- ── Paint helpers ─────────────────────────────────────────────────────────
--
-- Concept-aware painters built on the three Rust primitives
-- (frame:text, frame:fill, frame:box) plus the cursor state. Distros
-- can call these directly, override them, or replace them with their
-- own.
--
-- `paint.buffer` is the workhorse: it styles the selection per cell,
-- batches consecutive cells of the same style into runs, and emits one
-- `frame:text` call per visible line.

gauchito.paint = {}

-- Default style applied to characters outside the selection.
gauchito.paint.default_style = {}

-- Style applied to selection cells when `paint.buffer` is called with
-- an `opts.view` arg. Distros override by replacing this table.
gauchito.paint.selection_style = { bg = 'lightblue' }

-- ── Viewport ──────────────────────────────────────────────────────────────
--
-- Typed opts-bag for `gauchito.paint.*`. Defends against the
-- accumulating-fields drift (scroll_row, view, show_cursor,
-- cursor_style, ...) by centralizing defaults.
--
-- All fields are optional; missing keys take documented defaults.
-- `paint.buffer` and `paint.view` accept either a Viewport or a plain
-- table.

gauchito.viewport = {}

local Viewport = {}
Viewport.__index = Viewport

function gauchito.viewport.new(fields: ViewportFields?): Viewport
    fields = fields or {}
    return setmetatable({
        scroll_row     = fields.scroll_row or 0,
        scroll_col     = fields.scroll_col or 0,
        view           = fields.view,
        show_cursor    = fields.show_cursor ~= false,
        show_selection = fields.show_selection ~= false,
        cursor_style   = fields.cursor_style or 'block',
    }, Viewport)
end

local function selected(spans, pos)
    for _, span in ipairs(spans) do
        if pos >= span.from and pos < span.to then return true end
    end
    return false
end

-- Paint a buffer's visible lines into `area` as styled runs.
--
-- `opts.scroll_row` — first buffer line to draw (default 0).
-- `opts.view`       — optional View whose selection is styled with
--                     gauchito.paint.selection_style.
function gauchito.paint.buffer(buf: Buffer, area: Rect, opts: PaintBufferOpts?)
    opts         = opts or {}
    local scroll = opts.scroll_row or 0
    local frame  = gauchito.frame

    if area.h == 0 or area.w == 0 then return end

    local line_count = buf:line_count()
    if line_count == 0 then return end

    local visible_from = buf:bol(scroll)
    local last_visible_line = math.min(line_count - 1, scroll + area.h - 1)
    local visible_to = buf:right(buf:eol(last_visible_line))

    local spans = {}
    if opts.view ~= nil then
        for _, r in ipairs(opts.view.selection.ranges) do
            local f = math.min(r.anchor, r.head)
            local t = math.max(r.anchor, r.head)
            if f < t and f < visible_to and t > visible_from then
                spans[#spans + 1] = { from = f, to = t }
            end
        end
    end

    for row = 0, area.h - 1 do
        local line_n = scroll + row
        if line_n >= line_count then break end

        -- One cell per char, clipped to the area width; `pos` is the
        -- char's byte offset in the buffer.
        local chars, styles = {}, {}
        local pos = buf:bol(line_n)
        for c in buf:line(line_n):gmatch(utf8.charpattern) do
            if #chars == area.w then break end
            chars[#chars + 1] = c
            styles[#chars] = selected(spans, pos) and gauchito.paint.selection_style
                or gauchito.paint.default_style
            pos = pos + #c
        end

        -- Coalesce same-style runs.
        local runs = {}
        local i = 1
        while i <= #chars do
            local j = i
            while j < #chars and styles[j + 1] == styles[i] do j = j + 1 end
            runs[#runs + 1] = { table.concat(chars, '', i, j), styles[i] }
            i = j + 1
        end

        frame:text({ x = area.x, y = area.y + row, w = area.w, h = 1 }, runs)
    end
end

-- Position the terminal cursor at the view's primary head (if visible).
function gauchito.paint.cursor(view: View, area: Rect, opts: PaintCursorOpts?)
    opts = opts or {}
    local scroll = opts.scroll_row or 0
    local buf = view.buf
    local r = view.selection:primary_range()
    local p = r.head
    local line = buf:line_at(p)
    if line < scroll or line >= scroll + area.h then return end
    local col = buf:visual_col(p)
    gauchito.frame:set_cursor(
        area.x + col, area.y + (line - scroll), opts.cursor_style)
end

-- Composition: paint a view (buffer + selection + cursor).
function gauchito.paint.view(view: View, area: Rect, opts: PaintViewOpts?)
    opts = opts or {}
    local buffer_opts = {
        scroll_row = opts.scroll_row,
        view       = (opts.show_selection ~= false) and view or nil,
    }
    gauchito.paint.buffer(view.buf, area, buffer_opts)
    if opts.show_cursor then
        gauchito.paint.cursor(view, area, {
            scroll_row   = opts.scroll_row,
            cursor_style = opts.cursor_style,
        })
    end
end

-- Paint a popup: fill background + optional border. Returns the inner
-- content rect so callers paint into the right area.
function gauchito.paint.popup(area: Rect, opts: PaintPopupOpts?): Rect
    opts = opts or {}
    local frame = gauchito.frame
    local bg = opts.bg or { bg = 'darkgray' }
    frame:fill(area, ' ', bg)
    if opts.border then
        local b = opts.border == true and { fg = 'white' } or opts.border
        frame:box(area, opts.charset or 'rounded', b)
        return {
            x = area.x + 1,
            y = area.y + 1,
            w = math.max(0, area.w - 2),
            h = math.max(0, area.h - 2),
        }
    end
    return area
end
