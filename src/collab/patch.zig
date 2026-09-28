const std = @import("std");
const egwalker = @import("egwalker");
const splice = @import("../core/splice.zig");

// TODO turn a merged patch into the splices the buffer understands.
//   insert -> Splice(pos, pos, text)
//   delete -> Splice(pos, pos + len, "")
// The patch borrows its text from the oplog, and the oplog outlives the
// patch, so the splices can borrow too: no copies here.
// The returned slice is owned by the caller. An arena that gets reset once
// per merge is the allocator this wants, not the general purpose one.
pub fn splices(gpa: std.mem.Allocator, patch: *egwalker.Patch) ![]splice.Splice {
    _ = .{ gpa, patch };
    @panic("todo");
}

const testing = std.testing;

test "a patch of one insert becomes one splice" {
    var oplog: egwalker.OpLog = .init(testing.allocator, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello");

    var branch: egwalker.Branch = .init(testing.allocator);
    defer branch.deinit();
    var patch = try branch.merge(&oplog);
    defer patch.deinit();

    const out = try splices(testing.allocator, &patch);
    defer testing.allocator.free(out);

    try testing.expectEqual(@as(usize, 1), out.len);
    try testing.expectEqual(@as(u32, 0), out[0].p);
    try testing.expectEqual(@as(u32, 0), out[0].q);
    try testing.expectEqualStrings("hello", out[0].text);
}

test "a delete becomes a splice with an empty text" {
    var oplog: egwalker.OpLog = .init(testing.allocator, .{ .agent = 1 });
    defer oplog.deinit();
    try oplog.insert(0, "hello world");
    try oplog.delete(5, 6);

    var branch: egwalker.Branch = .init(testing.allocator);
    defer branch.deinit();
    var patch = try branch.merge(&oplog);
    defer patch.deinit();

    const out = try splices(testing.allocator, &patch);
    defer testing.allocator.free(out);

    try testing.expectEqual(@as(usize, 2), out.len);
    try testing.expectEqual(@as(u32, 5), out[1].p);
    try testing.expectEqual(@as(u32, 11), out[1].q);
    try testing.expectEqualStrings("", out[1].text);
}
