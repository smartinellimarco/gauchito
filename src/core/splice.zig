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
