--- Gauchito substrate API — type definitions for luau-lsp.
---
--- The `gauchito` global is injected by Rust; this file only describes its
--- shape so distros typecheck. Positions are character offsets; lines are
--- zero-indexed. Object types use `export type` (not `declare class`) so the
--- file is checkable by upstream `luau-analyze`, not just luau-lsp.

-- ── Plain value types ──────────────────────────────────────────────────────

export type Rect = { x: number, y: number, w: number, h: number }

export type Style = {
    fg: string?,
    bg: string?,
    bold: boolean?,
    italic: boolean?,
    underline: boolean?,
    reverse: boolean?,
}

--- A styled run for `Frame:text`: a bare string (unstyled), or a
--- `{ text, style }` table. The positional `{ "txt", style }` form is also
--- accepted at runtime.
export type Run = string | { text: string, style: Style? }

export type Range = { anchor: number, head: number }

export type CharClass = "eol" | "whitespace" | "word" | "punct"

--- A partial set of buffer options — any field may be absent. Layers merge
--- left-to-right (later wins) via `gauchito.options.merge`.
export type PartialBufferOptions = {
    line_ending: ("lf" | "crlf")?,
    final_newline: boolean?,
    bom: boolean?,
    trim_trailing_whitespace: boolean?,
    indent: { unit: string, tab_width: number }?,
}

--- Raw key event. `code` is one of: 'char' | 'enter' | 'esc' | 'backspace'
--- | 'del' | 'tab' | 'left' | 'right' | 'up' | 'down' | 'home' | 'end' |
--- 'pageup' | 'pagedown' | 'space' | 'f1'..'f12' | 'unknown'.
export type KeyEvent = {
    code: string,
    ch: string?, -- the character when code == 'char' or 'space'
    ctrl: boolean,
    alt: boolean,
    shift: boolean,
}

--- A named charset for `Frame:box`, or a custom set of corner/edge chars.
export type BoxCharset =
    "rounded"
    | "double"
    | "ascii"
    | { tl: string, tr: string, bl: string, br: string, h: string, v: string }

export type IoReadResult = { rope: Rope, sniffed: PartialBufferOptions }

-- ── Handle types (Rust userdata) ───────────────────────────────────────────

--- Opaque rope handle from `gauchito.io.read`; feed it to
--- `gauchito.buf.from_rope`, or query it directly.
export type Rope = {
    len: (self: Rope) -> number,
    line: (self: Rope, n: number) -> string,
}

--- The substrate edit primitive: replace `[p, q)` with `text`. Immutable;
--- build via `gauchito.splice.new`.
export type Splice = {
    p: number,
    q: number,
    text: string,
}

--- The editable, persistent thing: rope + pins + path +
--- options. Edit only through `apply` — the one path that reprojects pins.
export type Buffer = {
    id: number,
    path: string?,
    revision: number,
    name: string,
    modified: boolean,

    -- editing
    apply: (self: Buffer, splices: { Splice }) -> { Splice },
    save: (self: Buffer) -> (),

    -- queries
    len: (self: Buffer) -> number,
    line_count: (self: Buffer) -> number,
    char: (self: Buffer, pos: number) -> string,
    slice: (self: Buffer, from: number, to: number) -> string,
    line: (self: Buffer, n: number) -> string,
    bol: (self: Buffer, n: number) -> number,
    eol: (self: Buffer, n: number) -> number,
    line_at: (self: Buffer, pos: number) -> number,
    char_class: (self: Buffer, pos: number) -> CharClass,
    scan_class_fwd: (self: Buffer, pos: number) -> number,
    scan_class_bwd: (self: Buffer, pos: number) -> number,

    -- motion kernels (position -> position)
    left: (self: Buffer, pos: number) -> number,
    right: (self: Buffer, pos: number) -> number,
    left_inline: (self: Buffer, pos: number) -> number,
    right_inline: (self: Buffer, pos: number) -> number,
    up: (self: Buffer, pos: number) -> number,
    down: (self: Buffer, pos: number) -> number,
    line_start_of: (self: Buffer, pos: number) -> number,
    line_end_of: (self: Buffer, pos: number) -> number,
    first_non_ws: (self: Buffer, pos: number) -> number,
    doc_end: (self: Buffer) -> number,
    visual_col: (self: Buffer, pos: number) -> number,
    pos_at: (self: Buffer, line: number, col: number) -> number,
}

--- Immutable list of `(anchor, head)` ranges. Mutators return a new value.
export type Selection = {
    ranges: { Range },
    primary: number,
    len: (self: Selection) -> number,
    primary_idx: (self: Selection) -> number,
    primary_range: (self: Selection) -> Range,
    range: (self: Selection, i: number) -> Range,
    add_range: (self: Selection, anchor: number, head: number) -> Selection,
    remove: (self: Selection, idx: number) -> Selection,
    set_primary: (self: Selection, i: number) -> Selection,
}

--- A cursor session on a buffer. Pure substrate: identity + buf + selection.
export type View = {
    id: number,
    buf: Buffer,
    selection: Selection,
    set_selection: (self: View, sel: Selection) -> (),
    snapshot: (self: View) -> Selection,
    restore: (self: View, snap: Selection) -> (),
    set_buf: (self: View, buf: Buffer) -> (),
}

--- The persistent paint-op buffer. Distros mutate it; the host flushes it
--- after each event/wake.
export type Frame = {
    area: (self: Frame) -> Rect,
    set_area: (self: Frame, area: Rect) -> (),
    clear: (self: Frame) -> (),
    text: (self: Frame, area: Rect, runs: string | { Run }) -> (),
    fill: (self: Frame, area: Rect, ch: string, style: Style?) -> (),
    box: (self: Frame, area: Rect, charset: BoxCharset?, style: Style?) -> (),
    set_cursor: (self: Frame, x: number, y: number, style: ("block" | "bar")?) -> (),
}

--- Yieldable from async constructors (`gauchito.io.read`,
--- `gauchito.task.sleep`, `gauchito.buf.open`). `:await()` resumes the
--- calling coroutine with the awaited value, or raises on failure.
export type Awaitable = {
    await: (self: Awaitable) -> any,
}

--- Yieldable from `gauchito.keys.read()`. `:await()` resumes with
--- `(name, ch, event)`.
export type KeysAwaitable = {
    await: (self: KeysAwaitable) -> (string, string?, KeyEvent),
}

--- An event-bus subscription handle.
export type Handle = {
    cancel: (self: Handle) -> (),
}

-- ── Paint / viewport opts ──────────────────────────────────────────────────

--- Resolved typed opts-bag for `paint.*` (every field set).
export type Viewport = {
    scroll_row: number,
    scroll_col: number,
    view: View?,
    show_cursor: boolean,
    show_selection: boolean,
    cursor_style: "block" | "bar",
}

--- Partial input to `gauchito.viewport.new` (omitted fields take defaults).
export type ViewportFields = {
    scroll_row: number?,
    scroll_col: number?,
    view: View?,
    show_cursor: boolean?,
    show_selection: boolean?,
    cursor_style: ("block" | "bar")?,
}

export type PaintBufferOpts = {
    scroll_row: number?,
    view: View?, -- when set, its selection is styled with paint.selection_style
}

export type PaintCursorOpts = {
    scroll_row: number?,
    cursor_style: ("block" | "bar")?,
}

export type PaintViewOpts = {
    scroll_row: number?,
    show_selection: boolean?,
    show_cursor: boolean?,
    cursor_style: ("block" | "bar")?,
}

export type PaintPopupOpts = {
    bg: Style?,
    border: (boolean | Style)?,
    charset: BoxCharset?,
}

-- ── The injected global ─────────────────────────────────────────────────────

declare gauchito: {
    --- CLI args; argv[1] is the first positional.
    argv: { string },

    --- The editor version, for `--version` style flags.
    version: string,

    --- Exit on the next tick.
    quit: () -> (),

    --- The persistent paint buffer (a value, not a function).
    frame: Frame,

    --- Buffer constructors.
    buf: {
        scratch: (path: string?, opts: PartialBufferOptions?) -> Buffer,
        from_rope: (rope: Rope, path: string?, opts: PartialBufferOptions?) -> Buffer,
        open: (path: string, opts: PartialBufferOptions?) -> Awaitable,
    },

    --- View constructor.
    view: {
        new: (buf: Buffer) -> View,
    },

    --- Selection value constructors.
    selection: {
        new: (ranges: { Range }) -> Selection,
        point: (pos: number) -> Selection,
        range: (anchor: number, head: number) -> Range,
    },

    --- Edit-primitive constructor.
    splice: {
        new: (p: number, q: number, text: string) -> Splice,
    },

    --- Pull-based key dispatch.
    keys: {
        read: () -> KeysAwaitable,
        name: (event: KeyEvent) -> string,
    },

    --- Concept-aware painters built on the Frame primitives.
    paint: {
        buffer: (buf: Buffer, area: Rect, opts: PaintBufferOpts?) -> (),
        cursor: (view: View, area: Rect, opts: PaintCursorOpts?) -> (),
        view: (view: View, area: Rect, opts: PaintViewOpts?) -> (),
        popup: (area: Rect, opts: PaintPopupOpts?) -> Rect,
        default_style: Style,
        selection_style: Style,
    },

    --- Pure-Lua pubsub.
    event: {
        on: (name: string, fn: (payload: any) -> ()) -> Handle,
        once: (name: string, fn: (payload: any) -> ()) -> Handle,
        emit: (name: string, payload: any) -> (),
    },

    --- Coroutine spawn + awaitables.
    task: {
        spawn: (fn: () -> ()) -> { id: number },
        sleep: (ms: number) -> Awaitable,
    },

    --- Async file read.
    io: {
        read: (path: string) -> Awaitable,
    },

    --- Terminal size (pull-based).
    term: {
        size: () -> { w: number, h: number },
    },

    --- Option merge/resolve.
    options: {
        merge: (...PartialBufferOptions) -> PartialBufferOptions,
        resolve: (partial: PartialBufferOptions?) -> PartialBufferOptions,
    },

    --- Typed opts-bag builder for paint.*.
    viewport: {
        new: (fields: ViewportFields?) -> Viewport,
    },
}
