const std = @import("std");
const egwalker = @import("egwalker");
const splice = @import("../core/splice.zig");
const patch_mod = @import("patch.zig");

/// One collaborative document: the history and this replica's place in it.
/// It does not own the text — merging hands back splices and the buffer
/// applies them, so there is exactly one copy of the document in the editor.
pub const Doc = struct {
    gpa: std.mem.Allocator,
    oplog: egwalker.OpLog,
    branch: egwalker.Branch,

    // TODO init/deinit. The agent id identifies this replica in the history;
    // two replicas that share an id corrupt the document, so it comes from
    // the caller, not from a counter in here.
    pub fn init(gpa: std.mem.Allocator, agent: egwalker.AgentId) Doc {
        _ = .{ gpa, agent };
        @panic("todo");
    }

    pub fn deinit(self: *Doc) void {
        _ = self;
        @panic("todo");
    }

    // TODO record a local edit. Positions are characters, not bytes.
    pub fn insert(self: *Doc, pos: u32, text: []const u8) !void {
        _ = .{ self, pos, text };
        @panic("todo");
    }

    // TODO a delete run walks left when it is a backspace: the oplog cares,
    // because that is what makes a held backspace collapse into one run.
    pub fn delete(self: *Doc, pos: u32, count: u32) !void {
        _ = .{ self, pos, count };
        @panic("todo");
    }

    pub fn backspace(self: *Doc, pos: u32, count: u32) !void {
        _ = .{ self, pos, count };
        @panic("todo");
    }

    // TODO catch the branch up with everything the oplog knows and return the
    // splices to apply to the buffer, in order. Owned by the caller.
    // Both the patch and the splices are per-merge scratch: allocate them in
    // an arena you reset, not in the general purpose allocator.
    pub fn merge(self: *Doc, gpa: std.mem.Allocator) ![]splice.Splice {
        _ = .{ self, gpa };
        @panic("todo");
    }

    // TODO take bytes a peer sent and put them in the history. Nothing is
    // applied to the buffer until the next merge.
    pub fn receive(self: *Doc, bytes: []const u8) !void {
        _ = .{ self, bytes };
        @panic("todo");
    }
};

const testing = std.testing;

fn textOf(gpa: std.mem.Allocator, doc: *Doc, into: *egwalker.Text) ![]u8 {
    const out = try doc.merge(gpa);
    defer gpa.free(out);

    for (out) |s| {
        if (s.q > s.p) into.delete(s.p, s.q - s.p);
        if (s.text.len > 0) try into.insertUtf8(s.p, s.text);
    }

    return into.toUtf8(gpa);
}

test "local edits come back as splices that rebuild the text" {
    const gpa = testing.allocator;

    var doc: Doc = .init(gpa, 1);
    defer doc.deinit();

    try doc.insert(0, "hello world");
    try doc.delete(5, 6);
    try doc.insert(5, " there");

    var text: egwalker.Text = try .init(gpa);
    defer text.deinit();

    const out = try textOf(gpa, &doc, &text);
    defer gpa.free(out);
    try testing.expectEqualStrings("hello there", out);
}

test "merging twice in a row emits nothing the second time" {
    const gpa = testing.allocator;

    var doc: Doc = .init(gpa, 1);
    defer doc.deinit();
    try doc.insert(0, "hi");

    const first = try doc.merge(gpa);
    gpa.free(first);

    const second = try doc.merge(gpa);
    defer gpa.free(second);
    try testing.expectEqual(@as(usize, 0), second.len);
}

test "two replicas that exchange bytes converge" {
    const gpa = testing.allocator;

    var a: Doc = .init(gpa, 1);
    defer a.deinit();
    var b: Doc = .init(gpa, 2);
    defer b.deinit();

    try a.insert(0, "hello");
    const from_a = try a.oplog.serializeAll(gpa);
    defer gpa.free(from_a);
    try b.receive(from_a);

    // Both type at the same place without having seen each other.
    try a.insert(5, "!");
    try b.insert(5, "?");

    const a_bytes = try a.oplog.serializeAll(gpa);
    defer gpa.free(a_bytes);
    const b_bytes = try b.oplog.serializeAll(gpa);
    defer gpa.free(b_bytes);
    try a.receive(b_bytes);
    try b.receive(a_bytes);

    var text_a: egwalker.Text = try .init(gpa);
    defer text_a.deinit();
    var text_b: egwalker.Text = try .init(gpa);
    defer text_b.deinit();

    const out_a = try textOf(gpa, &a, &text_a);
    defer gpa.free(out_a);
    const out_b = try textOf(gpa, &b, &text_b);
    defer gpa.free(out_b);

    try testing.expectEqualStrings(out_a, out_b);
    try testing.expectEqual(@as(usize, 7), out_a.len);
}
