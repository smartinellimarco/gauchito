---@meta

---@class gauchito.Rect
---@field x integer
---@field y integer
---@field w integer
---@field h integer

---@class gauchito.Style
---@field fg? string
---@field bg? string
---@field bold? boolean
---@field italic? boolean
---@field underline? boolean
---@field reverse? boolean

---@alias gauchito.Run string | { [1]: string, [2]: gauchito.Style? } | { text: string, style: gauchito.Style? }

---@alias gauchito.Charset "rounded" | "double" | "ascii" | { tl: string, tr: string, bl: string, br: string, h: string, v: string }

---@class gauchito.Key
---@field code string char, space, enter, esc, backspace, del, tab, left, right, up, down, home, end, pageup, pagedown, f1..f12, unknown
---@field ch? string
---@field ctrl boolean
---@field alt boolean
---@field shift boolean

---@class gauchito.Range
---@field anchor integer
---@field head integer

---@class gauchito.Selection
---@field ranges gauchito.Range[]
---@field primary integer
local Selection = {}

---@return gauchito.Range
function Selection:primary_range() end

---@class gauchito.Splice

---@class gauchito.Buffer
---@field id integer
---@field modified boolean
local Buffer = {}

---@param splices gauchito.Splice[]
function Buffer:apply(splices) end
---@return integer
function Buffer:len() end
---@return integer
function Buffer:line_count() end
---@param pos integer
---@return string
function Buffer:char(pos) end
---@param from integer
---@param to integer
---@return string
function Buffer:slice(from, to) end
---@param n integer
---@return string
function Buffer:line(n) end
---@param n integer
---@return integer
function Buffer:bol(n) end
---@param n integer
---@return integer
function Buffer:eol(n) end
---@param pos integer
---@return integer
function Buffer:line_at(pos) end
---@param pos integer
---@return "eol" | "whitespace" | "word" | "punct"
function Buffer:char_class(pos) end
---@param pos integer
---@return integer
function Buffer:scan_class_fwd(pos) end
---@param pos integer
---@return integer
function Buffer:scan_class_bwd(pos) end
---@param pos integer
---@return integer
function Buffer:left(pos) end
---@param pos integer
---@return integer
function Buffer:right(pos) end
---@param pos integer
---@return integer
function Buffer:left_inline(pos) end
---@param pos integer
---@return integer
function Buffer:right_inline(pos) end
---@param pos integer
---@return integer
function Buffer:up(pos) end
---@param pos integer
---@return integer
function Buffer:down(pos) end
---@param pos integer
---@return integer
function Buffer:line_start_of(pos) end
---@param pos integer
---@return integer
function Buffer:line_end_of(pos) end
---@param pos integer
---@return integer
function Buffer:first_non_ws(pos) end
---@param pos integer
---@return integer
function Buffer:visual_col(pos) end
---@return integer
function Buffer:doc_end() end
---@param line integer
---@param col integer
---@return integer
function Buffer:pos_at(line, col) end

---@class gauchito.View
---@field id integer
---@field buf gauchito.Buffer
---@field selection gauchito.Selection
local View = {}

---@param selection gauchito.Selection
function View:set_selection(selection) end

---@class gauchito.Frame
local Frame = {}

---@return gauchito.Rect
function Frame:area() end
---@param area gauchito.Rect
function Frame:set_area(area) end
function Frame:clear() end
---@param area gauchito.Rect
---@param runs string | gauchito.Run[]
function Frame:text(area, runs) end
---@param area gauchito.Rect
---@param ch string
---@param style? gauchito.Style
function Frame:fill(area, ch, style) end
---@param area gauchito.Rect
---@param charset? gauchito.Charset
---@param style? gauchito.Style
function Frame:box(area, charset, style) end
---@param x integer
---@param y integer
---@param style? "block" | "bar"
function Frame:set_cursor(x, y, style) end

gauchito = {}

---@type string
gauchito.version = nil

---@type string[]
gauchito.argv = {}

---@type gauchito.Frame
gauchito.frame = nil

function gauchito.quit() end

gauchito.term = {}

---@return { w: integer, h: integer }
function gauchito.term.size() end

gauchito.buf = {}

---@return gauchito.Buffer
function gauchito.buf.new() end

gauchito.view = {}

---@param buf gauchito.Buffer
---@return gauchito.View
function gauchito.view.new(buf) end

gauchito.selection = {}

---@param ranges gauchito.Range[]
---@return gauchito.Selection
function gauchito.selection.new(ranges) end

gauchito.splice = {}

---@param p integer
---@param q integer
---@param text string
---@return gauchito.Splice
function gauchito.splice.new(p, q, text) end

gauchito.task = {}

---@param fn fun()
function gauchito.task.spawn(fn) end

---@async
---@param ms integer
function gauchito.sleep(ms) end

gauchito.keys = {}

---@async
---@return gauchito.Key
function gauchito.keys.read() end
