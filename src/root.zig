pub const collab = struct {
    pub const doc = @import("collab/doc.zig");
    pub const map = @import("collab/map.zig");
    pub const patch = @import("collab/patch.zig");
    pub const sync = @import("collab/sync.zig");
};

pub const core = struct {
    pub const splice = @import("core/splice.zig");
};

// Tests only run for files the compilation pulls in, so every file is listed.
test {
    _ = @import("collab/doc.zig");
    _ = @import("collab/map.zig");
    _ = @import("collab/patch.zig");
    _ = @import("collab/sync.zig");
    _ = @import("core/splice.zig");
}
