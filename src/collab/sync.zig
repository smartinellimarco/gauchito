const std = @import("std");
const egwalker = @import("egwalker");
const doc_mod = @import("doc.zig");

// The handshake, in three messages and no sockets. Who carries the bytes is
// the caller's problem: a socket, a file, a pipe, a test.
//
//   1. each side sends what it has          summary()
//   2. each side answers with what is missing   missing()
//   3. each side swallows the answer         Doc.receive()

// TODO what this replica has, expressed per agent and sequence so the two
// sides never have to share local version numbers.
pub fn summary(gpa: std.mem.Allocator, doc: *const doc_mod.Doc) ![]u8 {
    _ = .{ gpa, doc };
    @panic("todo");
}

// TODO given the other side's summary, serialize only the events it lacks.
// Returns an empty slice when there is nothing to send, which is the common
// case and should not allocate a message.
pub fn missing(gpa: std.mem.Allocator, doc: *const doc_mod.Doc, their_summary: []const u8) ![]u8 {
    _ = .{ gpa, doc, their_summary };
    @panic("todo");
}

const testing = std.testing;

test "only the missing events travel" {
    const gpa = testing.allocator;

    var a: doc_mod.Doc = .init(gpa, 1);
    defer a.deinit();
    var b: doc_mod.Doc = .init(gpa, 2);
    defer b.deinit();

    try a.insert(0, "hello");

    const theirs = try summary(gpa, &b);
    defer gpa.free(theirs);

    const delta = try missing(gpa, &a, theirs);
    defer gpa.free(delta);
    try b.receive(delta);

    try testing.expectEqual(a.oplog.len(), b.oplog.len());
}

test "nothing to send once both sides agree" {
    const gpa = testing.allocator;

    var a: doc_mod.Doc = .init(gpa, 1);
    defer a.deinit();
    var b: doc_mod.Doc = .init(gpa, 2);
    defer b.deinit();

    try a.insert(0, "hello");

    const first = try summary(gpa, &b);
    defer gpa.free(first);
    const delta = try missing(gpa, &a, first);
    defer gpa.free(delta);
    try b.receive(delta);

    const second = try summary(gpa, &b);
    defer gpa.free(second);
    const nothing = try missing(gpa, &a, second);
    defer gpa.free(nothing);

    try testing.expectEqual(@as(usize, 0), nothing.len);
}

test "a summary survives the round trip through bytes" {
    const gpa = testing.allocator;

    var a: doc_mod.Doc = .init(gpa, 1);
    defer a.deinit();
    try a.insert(0, "hello");
    try a.delete(1, 2);

    const bytes = try summary(gpa, &a);
    defer gpa.free(bytes);

    var decoded = try egwalker.encoding.sync.VersionSummary.decode(gpa, bytes);
    defer decoded.deinit();

    try testing.expect(decoded.contains(1, 0));
    try testing.expect(!decoded.contains(1, 99));
}
