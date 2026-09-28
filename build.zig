const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const egwalker = b.dependency("egwalker", .{ .target = target, .optimize = optimize });
    const rope = b.dependency("rope", .{ .target = target, .optimize = optimize });
    const zlua = b.dependency("zlua", .{ .target = target, .optimize = optimize });
    const vaxis = b.dependency("vaxis", .{ .target = target, .optimize = optimize });

    const mod = b.addModule("gauchito", .{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
        .imports = &.{
            .{ .name = "egwalker", .module = egwalker.module("egwalker") },
            .{ .name = "rope", .module = rope.module("rope") },
            .{ .name = "zlua", .module = zlua.module("zlua") },
            .{ .name = "vaxis", .module = vaxis.module("vaxis") },
        },
    });

    const exe = b.addExecutable(.{
        .name = "gauchito",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{.{ .name = "gauchito", .module = mod }},
        }),
    });
    b.installArtifact(exe);

    const run_step = b.step("run", "Run the editor");
    const run = b.addRunArtifact(exe);
    if (b.args) |args| run.addArgs(args);
    run_step.dependOn(&run.step);

    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&b.addRunArtifact(b.addTest(.{ .root_module = mod })).step);
}
