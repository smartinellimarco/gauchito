const std = @import("std");

pub const Gravity = enum { left, right };

/// Replace [p, q) with text. The unit of edit and the unit of history.
pub const Splice = struct {
    p: u32,
    q: u32,
    text: []const u8,

    // TODO asserts p <= q.
    pub fn init(p: u32, q: u32, text: []const u8) Splice {
        _ = .{ p, q, text };
        @panic("todo");
    }

    // TODO true when it neither deletes nor inserts.
    pub fn isIdentity(self: Splice) bool {
        _ = self;
        @panic("todo");
    }
};

// TODO where does offset r land after s?
//   r < s.p                 unchanged
//   r <= s.q                collapses: left -> s.p, right -> s.p + chars(text)
//   otherwise               shifted by chars(text) - (s.q - s.p)
// Lengths are in characters, not bytes.
pub fn phi(s: Splice, r: u32, gravity: Gravity) u32 {
    _ = .{ s, r, gravity };
    @panic("todo");
}

// TODO move one position through a run of splices, in order. This is the only
// projection in the editor: the splices come from your own edit or from a
// merge, and pins, selections and decorations all ride it.
pub fn position(pos: u32, gravity: Gravity, splices: []const Splice) u32 {
    _ = .{ pos, gravity, splices };
    @panic("todo");
}

// TODO the same for many positions in one pass over the splices instead of one
// pass per position. Rewrites in place.
// Careful: the result is no longer sorted when two positions with opposite
// gravity collapse onto the same splice.
pub fn positions(slots: []u32, gravity: Gravity, splices: []const Splice) void {
    _ = .{ slots, gravity, splices };
    @panic("todo");
}

const testing = std.testing;
const left: Gravity = .left;
const right: Gravity = .right;

test "phi leaves a position before the splice alone" {
    try testing.expectEqual(@as(u32, 3), phi(.init(5, 5, "XY"), 3, right));
    try testing.expectEqual(@as(u32, 2), phi(.init(3, 7, ""), 2, right));
}

test "phi pushes a position at a pure insert" {
    const s: Splice = .init(5, 5, "XY");
    try testing.expectEqual(@as(u32, 7), phi(s, 5, right));
    try testing.expectEqual(@as(u32, 10), phi(s, 8, right));
}

test "phi collapses a position inside a delete" {
    const s: Splice = .init(3, 7, "");
    try testing.expectEqual(@as(u32, 3), phi(s, 3, right));
    try testing.expectEqual(@as(u32, 3), phi(s, 5, right));
    try testing.expectEqual(@as(u32, 3), phi(s, 7, right));
    try testing.expectEqual(@as(u32, 5), phi(s, 9, right));
}

test "phi through a replace that shrinks" {
    const s: Splice = .init(3, 7, "AB");
    try testing.expectEqual(@as(u32, 2), phi(s, 2, right));
    try testing.expectEqual(@as(u32, 5), phi(s, 3, right));
    try testing.expectEqual(@as(u32, 5), phi(s, 7, right));
    try testing.expectEqual(@as(u32, 7), phi(s, 9, right));
}

test "phi through a replace that grows" {
    const s: Splice = .init(3, 5, "ABCD");
    try testing.expectEqual(@as(u32, 2), phi(s, 2, right));
    try testing.expectEqual(@as(u32, 7), phi(s, 3, right));
    try testing.expectEqual(@as(u32, 7), phi(s, 5, right));
    try testing.expectEqual(@as(u32, 10), phi(s, 8, right));
}

test "left gravity keeps a position at the start of the splice" {
    const insert: Splice = .init(5, 5, "XY");
    try testing.expectEqual(@as(u32, 5), phi(insert, 5, left));
    try testing.expectEqual(@as(u32, 10), phi(insert, 8, left));

    const replace: Splice = .init(3, 7, "AB");
    try testing.expectEqual(@as(u32, 3), phi(replace, 3, left));
    try testing.expectEqual(@as(u32, 3), phi(replace, 5, left));
    try testing.expectEqual(@as(u32, 3), phi(replace, 7, left));
}

test "multi byte characters count as one position" {
    const s: Splice = .init(2, 2, "áé→");
    try testing.expectEqual(@as(u32, 5), phi(s, 2, right));
}

test "an identity splice is the one that changes nothing" {
    try testing.expect(Splice.init(4, 4, "").isIdentity());
    try testing.expect(!Splice.init(4, 4, "x").isIdentity());
    try testing.expect(!Splice.init(4, 5, "").isIdentity());
}

test "a position before every splice stays where it is" {
    const splices = [_]Splice{ .init(10, 12, "AB"), .init(20, 20, "C") };
    try testing.expectEqual(@as(u32, 4), position(4, right, &splices));
}

test "splices apply in order, each one on the result of the last" {
    // "hello world" -> delete "world" -> insert "there": the second splice
    // speaks about positions that only exist after the first one ran.
    const splices = [_]Splice{ .init(6, 11, ""), .init(6, 6, "there") };
    try testing.expectEqual(@as(u32, 11), position(6, right, &splices));
    try testing.expectEqual(@as(u32, 2), position(2, right, &splices));
}

test "gravity decides what happens at a splice that starts on the position" {
    const splices = [_]Splice{.init(3, 3, "XY")};
    try testing.expectEqual(@as(u32, 3), position(3, left, &splices));
    try testing.expectEqual(@as(u32, 5), position(3, right, &splices));
}

test "a delete that covers the position leaves it on the start of the hole" {
    const splices = [_]Splice{.init(2, 8, "")};
    try testing.expectEqual(@as(u32, 2), position(5, right, &splices));
    try testing.expectEqual(@as(u32, 2), position(5, left, &splices));
}

test "many positions move in one pass" {
    const splices = [_]Splice{ .init(0, 0, "XY"), .init(9, 12, "") };
    var cursors = [_]u32{ 0, 4, 20 };
    positions(&cursors, right, &splices);
    try testing.expectEqualSlices(u32, &.{ 2, 6, 19 }, &cursors);
}
