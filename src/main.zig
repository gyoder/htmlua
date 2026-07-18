const std = @import("std");
const Io = std.Io;
const File = Io.File;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const ArrayList = std.ArrayList;
const StringArrayHashMap = std.StringArrayHashMapUnmanaged;

const HTMLua = @import("root.zig");

pub fn main(init: std.process.Init) !void {
    const io = init.io;

    var arena = std.heap.ArenaAllocator.init(init.gpa);
    defer arena.deinit();
    const alloc = arena.allocator();

    var state: HTMLua.Parser = .{
        .component_resolver = &resolver,
        .tags = &[_]HTMLua.ParserTag{
            HTMLua.tags.Include,
            HTMLua.tags.Export,
            HTMLua.tags.Use,
        },
        .tag_stack = try ArrayList([]const u8).initCapacity(alloc, 256),
        .exported_parts = try StringArrayHashMap([]const u8).init(alloc, &[0][]const u8{}, &[0][]const u8{}),
    };

    // I dont like this but this is temporary so its ok
    const args = try init.minimal.args.toSlice(alloc);
    const file_path = if (args.len != 1) args[1] else null;
    const file = if (file_path) |fp|
        try Io.Dir.cwd().openFile(io, fp, .{ .mode = .read_only })
    else
        null;
    defer if (file) |f| f.close(io);

    const in_buf = try alloc.alloc(u8, 1024);
    var in =
        if (file) |f| f.reader(io, in_buf) else File.stdin().reader(io, in_buf);

    const out_buf = try alloc.alloc(u8, 1024);
    var out = File.stdout().writer(io, out_buf);

    try HTMLua.parseTemplate(&arena, io, &state, &in.interface, &out.interface);

    try out.flush();
}

// https://ziggit.dev/t/zig-0-15-1-reader-writer-dont-make-copies-of-fieldparentptr-based-interfaces/11719
fn resolver(arena: *ArenaAllocator, io: Io, path: []const u8) anyerror!*Io.Reader {
    const read_buff = try arena.allocator().alloc(u8, 1024);
    const cwd = std.Io.Dir.cwd();

    var file = try cwd.openFile(io, path, .{ .mode = .read_only });
    const reader = try arena.allocator().create(Io.File.Reader);
    reader.* = file.reader(io, read_buff);
    return &reader.interface;
}
