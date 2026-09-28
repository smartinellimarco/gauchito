const std = @import("std");
const egwalker = @import("egwalker");

pub const Bias = egwalker.Bias;

// TODO move one position through a merged patch. This is how a cursor
// survives somebody else's edit: nothing is stored between merges, the
// position rides the operations that just arrived.
// Bias decides what happens when the edit lands exactly on the position:
// before keeps it in place, after pushes it along.
pub fn position(pos: u32, bias: Bias, patch: *egwalker.Patch) u32 {
    _ = .{ pos, bias, patch };
    @panic("todo");
}

// TODO the same for a whole sorted run of positions, in one pass over the
// patch instead of one pass per position. Rewrites in place.
// Careful: the result is not sorted anymore when two positions with
// opposite bias collapse onto the same edit.
pub fn positions(slots: []u32, bias: Bias, patch: *egwalker.Patch) void {
    _ = .{ slots, bias, patch };
    @panic("todo");
}

const testing = std.testing;

fn patchOf(gpa: std.mem.Allocator, oplog: *egwalker.OpLog, branch: *egwalker.Branch) !egwalker.Patch {
    _ = gpa;
    return branch.merge(oplog);
}

test "an insert before the cursor pushes it along" {
    const gpa = testing.allocator;

    var oplog: egwalker.OpLog = .init(gpa, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello");

    var branch: egwalker.Branch = .init(gpa);
    defer branch.deinit();
    var seen = try patchOf(gpa, &oplog, &branch);
    seen.deinit();

    try oplog.insert(0, "XY");
    var patch = try patchOf(gpa, &oplog, &branch);
    defer patch.deinit();

    try testing.expectEqual(@as(u32, 5), position(3, .after, &patch));
}

test "an insert after the cursor leaves it alone" {
    const gpa = testing.allocator;

    var oplog: egwalker.OpLog = .init(gpa, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello");

    var branch: egwalker.Branch = .init(gpa);
    defer branch.deinit();
    var seen = try patchOf(gpa, &oplog, &branch);
    seen.deinit();

    try oplog.insert(5, "XY");
    var patch = try patchOf(gpa, &oplog, &branch);
    defer patch.deinit();

    try testing.expectEqual(@as(u32, 3), position(3, .after, &patch));
}

test "a delete that covers the cursor lands it on the start of the hole" {
    const gpa = testing.allocator;

    var oplog: egwalker.OpLog = .init(gpa, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello world");

    var branch: egwalker.Branch = .init(gpa);
    defer branch.deinit();
    var seen = try patchOf(gpa, &oplog, &branch);
    seen.deinit();

    try oplog.delete(2, 6);
    var patch = try patchOf(gpa, &oplog, &branch);
    defer patch.deinit();

    try testing.expectEqual(@as(u32, 2), position(5, .after, &patch));
}

test "bias decides what happens at the edit itself" {
    const gpa = testing.allocator;

    var oplog: egwalker.OpLog = .init(gpa, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello");

    var branch: egwalker.Branch = .init(gpa);
    defer branch.deinit();
    var seen = try patchOf(gpa, &oplog, &branch);
    seen.deinit();

    try oplog.insert(3, "XY");
    var patch = try patchOf(gpa, &oplog, &branch);
    defer patch.deinit();

    try testing.expectEqual(@as(u32, 3), position(3, .before, &patch));
    patch.reset();
    try testing.expectEqual(@as(u32, 5), position(3, .after, &patch));
}

test "many positions move in one pass" {
    const gpa = testing.allocator;

    var oplog: egwalker.OpLog = .init(gpa, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello world");

    var branch: egwalker.Branch = .init(gpa);
    defer branch.deinit();
    var seen = try patchOf(gpa, &oplog, &branch);
    seen.deinit();

    try oplog.insert(0, "XY");
    var patch = try patchOf(gpa, &oplog, &branch);
    defer patch.deinit();

    var cursors = [_]u32{ 0, 4, 9 };
    positions(&cursors, .after, &patch);
    try testing.expectEqualSlices(u32, &.{ 2, 6, 11 }, &cursors);
}
