-- Default preset, loaded when there is no init.lua.
--
--   gauchito [path]      open path; a missing file opens empty and saves there
--   gauchito --version   print the version and exit
--
-- Arrows move, typing inserts, enter/backspace/del edit, ctrl-s saves,
-- ctrl-q quits.

local splice = gauchito.splice

local view = nil
local scroll_row = 0
local message = nil -- last error, shown in the statusline until the next key

local function content_area()
    local a = gauchito.frame:area()
    return { x = a.x, y = a.y, w = a.w, h = math.max(0, a.h - 1) }
end

local function statusline_area()
    local a = gauchito.frame:area()
    return { x = a.x, y = a.y + math.max(0, a.h - 1), w = a.w, h = 1 }
end

local function clamp_scroll_to_cursor()
    local h = content_area().h
    if h == 0 then return end
    local line = view.buf:line_at(view.selection:primary_range().head)
    if line < scroll_row then
        scroll_row = line
    elseif line >= scroll_row + h then
        scroll_row = line - h + 1
    end
end

local function paint_statusline()
    local buf = view.buf
    local head = view.selection:primary_range().head
    local text = string.format('  %s%s  %d:%d  %s',
        buf.path or '[scratch]',
        buf.modified and ' [+]' or '',
        buf:line_at(head) + 1,
        buf:visual_col(head) + 1,
        message or '')
    gauchito.frame:text(statusline_area(), { { text, { fg = 'black', bg = 'gray' } } })
end

local function redraw()
    local s = gauchito.term.size()
    gauchito.frame:set_area({ x = 0, y = 0, w = s.w, h = s.h })

    clamp_scroll_to_cursor()
    gauchito.frame:clear()
    gauchito.paint.view(view, content_area(), {
        scroll_row   = scroll_row,
        show_cursor  = true,
        cursor_style = 'bar',
    })
    paint_statusline()
end

local function move(kernel)
    local buf = view.buf
    local ranges = {}
    for _, r in ipairs(view.selection.ranges) do
        local p = kernel(buf, r.head)
        ranges[#ranges + 1] = { anchor = p, head = p }
    end
    view:set_selection(gauchito.selection.new(ranges))
end

-- Splices go right to left so each one's offsets are still valid when applied.
local function edit(build)
    local buf = view.buf
    local ranges = view.selection.ranges
    local splices = {}
    for i = #ranges, 1, -1 do
        splices[#splices + 1] = build(buf, ranges[i].head)
    end
    buf:apply(splices)
end

local function insert(text)
    edit(function(_, h) return splice.new(h, h, text) end)
end

local function line_end(b, p)
    local e = b:line_end_of(p)
    if b:char_class(e) == 'eol' then return e end
    return b:right(e)
end

local function save()
    view.buf:save()
end

local keys = {
    left      = function() move(function(b, p) return b:left(p) end) end,
    right     = function() move(function(b, p) return b:right(p) end) end,
    up        = function() move(function(b, p) return b:up(p) end) end,
    down      = function() move(function(b, p) return b:down(p) end) end,
    home      = function() move(function(b, p) return b:line_start_of(p) end) end,
    ['end']   = function() move(line_end) end,
    enter     = function() insert('\n') end,
    tab       = function() insert('\t') end,
    backspace = function() edit(function(b, h) return splice.new(b:left(h), h, '') end) end,
    del       = function() edit(function(b, h) return splice.new(h, b:right(h), '') end) end,
    ['ctrl-s'] = save,
    ['ctrl-q'] = gauchito.quit,
}

local function route(key, ch)
    local handler = keys[key]
    if handler then
        handler()
    elseif ch then
        insert(ch)
    end
end

local path = nil
for _, arg in ipairs(gauchito.argv) do
    if arg == '--version' then
        print(gauchito.version)
        gauchito.quit()
        return
    end
    path = arg
end

gauchito.event.on('task.error', function(payload)
    message = payload.message
    if view then redraw() end
end)

gauchito.task.spawn(function()
    local ok, buf = pcall(function() return path and gauchito.buf.open(path):await() end)
    if not ok then message = tostring(buf) end
    if not ok or not buf then buf = gauchito.buf.scratch(path) end
    view = gauchito.view.new(buf)
    redraw()

    while true do
        local key, ch = gauchito.keys.read():await()
        message = nil
        local ok, err = pcall(route, key, ch)
        if not ok then message = tostring(err) end
        redraw()
    end
end)
