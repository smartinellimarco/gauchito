const std = @import("std");
const splice_mod = @import("splice.zig");

const Splice = splice_mod.Splice;
const Gravity = splice_mod.Gravity;

/// Characters this transaction inserted, and what they were, so a redo can put
/// them back. `to - from` always equals the character count of `text`.
pub const Insertion = struct {
    from: u32,
    to: u32,
    text: []const u8,
};

/// Text this transaction removed, and where it was.
pub const Removal = struct {
    at: u32,
    text: []const u8,
};

pub const Region = struct {
    anchor: u32,
    head: u32,
};

pub const Snapshot = struct {
    regions: []const Region,
    primary: usize,
};

pub const Transaction = struct {
    insertions: []const Insertion,
    removals: []const Removal,
    before: Snapshot,
    after: Snapshot,
};

/// What to do to the document to move one step through the history.
pub const Step = struct {
    splices: []const Splice,
    selection: Snapshot,
};

/// An undo tree: branches survive a redo followed by an edit, so an abandoned
/// branch stays reachable.
///
/// Nothing in here touches the buffer. `undo` hands back splices and the caller
/// puts them through the same path as a keystroke, which is what makes an undo
/// a real edit that the other replicas see.
pub const History = struct {
    gpa: std.mem.Allocator,

    // TODO init/deinit. The history owns the text of every insertion and
    // removal, so `commit` copies it: the caller's buffer will have moved on
    // by the time an undo needs those bytes.
    pub fn init(gpa: std.mem.Allocator) History {
        _ = gpa;
        @panic("todo");
    }

    pub fn deinit(self: *History) void {
        _ = self;
        @panic("todo");
    }

    // TODO append a revision as a child of the current one and point at it.
    pub fn commit(self: *History, transaction: Transaction) !void {
        _ = .{ self, transaction };
        @panic("todo");
    }

    // TODO walk one step back: the splices that remove what the current
    // revision inserted and put back what it removed, plus the selection from
    // before the edit. Null when there is nothing left to undo.
    //
    // The splices come back in descending position, so applying them in order
    // never invalidates the next one.
    pub fn undo(self: *History, gpa: std.mem.Allocator) !?Step {
        _ = .{ self, gpa };
        @panic("todo");
    }

    // TODO the same forwards, through the child the last commit came from.
    pub fn redo(self: *History, gpa: std.mem.Allocator) !?Step {
        _ = .{ self, gpa };
        @panic("todo");
    }

    // TODO every merge moves the whole history: insertions, removals and
    // selection snapshots all ride `position`.
    //
    // One extra rule beyond the projection: an insert that lands strictly
    // inside an insertion splits it in two, text included. A run stands for
    // the characters of one transaction, and anything inserted later is not
    // one of them, whoever typed it. That is what keeps an undo from eating
    // somebody else's word that landed in the middle of yours.
    pub fn project(self: *History, splices: []const Splice) !void {
        _ = .{ self, splices };
        @panic("todo");
    }
};

const testing = std.testing;
const right: Gravity = .right;

fn snapshot(at: u32) Snapshot {
    return .{ .regions = &.{.{ .anchor = at, .head = at }}, .primary = 0 };
}

test "undoing an insert gives back the splice that removes it" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try history.commit(.{
        .insertions = &.{.{ .from = 0, .to = 5, .text = "hello" }},
        .removals = &.{},
        .before = snapshot(0),
        .after = snapshot(5),
    });

    const step = (try history.undo(testing.allocator)).?;
    defer testing.allocator.free(step.splices);

    try testing.expectEqual(@as(usize, 1), step.splices.len);
    try testing.expectEqual(@as(u32, 0), step.splices[0].p);
    try testing.expectEqual(@as(u32, 5), step.splices[0].q);
    try testing.expectEqualStrings("", step.splices[0].text);
    try testing.expectEqual(@as(u32, 0), step.selection.regions[0].head);
}

test "undoing a delete puts the text back where it was" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try history.commit(.{
        .insertions = &.{},
        .removals = &.{.{ .at = 3, .text = "abc" }},
        .before = snapshot(6),
        .after = snapshot(3),
    });

    const step = (try history.undo(testing.allocator)).?;
    defer testing.allocator.free(step.splices);

    try testing.expectEqual(@as(u32, 3), step.splices[0].p);
    try testing.expectEqual(@as(u32, 3), step.splices[0].q);
    try testing.expectEqualStrings("abc", step.splices[0].text);
}

test "an edit before mine moves what the undo will remove" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try history.commit(.{
        .insertions = &.{.{ .from = 0, .to = 5, .text = "hello" }},
        .removals = &.{},
        .before = snapshot(0),
        .after = snapshot(5),
    });

    // Somebody typed two characters at the start of the document.
    try history.project(&.{Splice.init(0, 0, "XY")});

    const step = (try history.undo(testing.allocator)).?;
    defer testing.allocator.free(step.splices);

    try testing.expectEqual(@as(u32, 2), step.splices[0].p);
    try testing.expectEqual(@as(u32, 7), step.splices[0].q);
    try testing.expectEqual(@as(u32, 2), step.selection.regions[0].head);
}

test "an insert inside my run splits it, and the undo takes only my characters" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try history.commit(.{
        .insertions = &.{.{ .from = 0, .to = 5, .text = "hello" }},
        .removals = &.{},
        .before = snapshot(0),
        .after = snapshot(5),
    });

    // "he" + "XY" + "llo": the word in the middle is not mine.
    try history.project(&.{Splice.init(2, 2, "XY")});

    const step = (try history.undo(testing.allocator)).?;
    defer testing.allocator.free(step.splices);

    // Descending, so applying them in order keeps the second one valid.
    try testing.expectEqual(@as(usize, 2), step.splices.len);
    try testing.expectEqual(@as(u32, 4), step.splices[0].p);
    try testing.expectEqual(@as(u32, 7), step.splices[0].q);
    try testing.expectEqual(@as(u32, 0), step.splices[1].p);
    try testing.expectEqual(@as(u32, 2), step.splices[1].q);
}

test "redo puts back what the undo removed" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try history.commit(.{
        .insertions = &.{.{ .from = 3, .to = 8, .text = "there" }},
        .removals = &.{},
        .before = snapshot(3),
        .after = snapshot(8),
    });

    const undone = (try history.undo(testing.allocator)).?;
    testing.allocator.free(undone.splices);

    // The undo is an edit like any other, so it comes back through project.
    try history.project(&.{Splice.init(3, 8, "")});

    const step = (try history.redo(testing.allocator)).?;
    defer testing.allocator.free(step.splices);

    try testing.expectEqual(@as(u32, 3), step.splices[0].p);
    try testing.expectEqual(@as(u32, 3), step.splices[0].q);
    try testing.expectEqualStrings("there", step.splices[0].text);
    try testing.expectEqual(@as(u32, 8), step.selection.regions[0].head);
}

test "an abandoned branch stays reachable after a redo and an edit" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try history.commit(.{
        .insertions = &.{.{ .from = 0, .to = 1, .text = "a" }},
        .removals = &.{},
        .before = snapshot(0),
        .after = snapshot(1),
    });

    const undone = (try history.undo(testing.allocator)).?;
    testing.allocator.free(undone.splices);
    try history.project(&.{Splice.init(0, 1, "")});

    // A different edit from the same place forks the tree instead of erasing.
    try history.commit(.{
        .insertions = &.{.{ .from = 0, .to = 1, .text = "b" }},
        .removals = &.{},
        .before = snapshot(0),
        .after = snapshot(1),
    });

    const step = (try history.undo(testing.allocator)).?;
    defer testing.allocator.free(step.splices);
    try testing.expectEqual(@as(u32, 1), step.splices[0].q);

    // "a" is still in the tree: redo reaches the branch the last commit came
    // from, so it comes back rather than being lost.
    const forward = (try history.redo(testing.allocator)).?;
    defer testing.allocator.free(forward.splices);
    try testing.expectEqualStrings("b", forward.splices[0].text);
}

test "nothing to undo on an empty history" {
    var history: History = .init(testing.allocator);
    defer history.deinit();

    try testing.expect(try history.undo(testing.allocator) == null);
    try testing.expect(try history.redo(testing.allocator) == null);
}
