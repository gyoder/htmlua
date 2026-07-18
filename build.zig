const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const ezts = b.dependency("eazy_zig_tree_sitter", .{
        .target = target,
        .optimize = optimize,
        .use_all_langs = true,
        .use_third_party = true,
    });

    const ts = b.dependency("tree_sitter", .{
        .target = target,
        .optimize = optimize,
    });

    const mod = b.addModule("htmlua", .{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .imports = &.{
            .{ .name = "easy-zig-tree-sitter", .module = ezts.module("eazy_zig_tree_sitter") },
            .{ .name = "tree-sitter", .module = ts.module("tree_sitter") },
        },
    });

    // Debug bin
    const exe = b.addExecutable(.{
        .name = "htmlua",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "htmlua", .module = mod },
            },
        }),
    });

    b.installArtifact(exe);

    const run_step = b.step("run", "Run the app");

    const run_cmd = b.addRunArtifact(exe);
    run_step.dependOn(&run_cmd.step);

    run_cmd.step.dependOn(b.getInstallStep());

    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    // CGI bin
    const cgi = b.addExecutable(.{
        .name = "htmlua-cgi",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/cgi.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "htmlua", .module = mod },
            },
        }),
    });

    b.installArtifact(cgi);

    const mod_tests = b.addTest(.{
        .root_module = mod,
    });

    const run_mod_tests = b.addRunArtifact(mod_tests);

    const exe_tests = b.addTest(.{
        .root_module = exe.root_module,
    });

    const run_exe_tests = b.addRunArtifact(exe_tests);

    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&run_mod_tests.step);
    test_step.dependOn(&run_exe_tests.step);

    // https://zigtools.org/zls/guides/build-on-save/
    const exe_check = b.addExecutable(.{
        .name = "htmlua",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "htmlua", .module = mod },
            },
        }),
    });
    const cgi_check = b.addExecutable(.{
        .name = "htmlua-cgi",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/cgi.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{
                .{ .name = "htmlua", .module = mod },
            },
        }),
    });

    const check = b.step("check", "Check if htmlua compiles");
    check.dependOn(&exe_check.step);
    check.dependOn(&cgi_check.step);
}
