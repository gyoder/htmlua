const std = @import("std");
const Io = std.Io;
const File = Io.File;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const ArrayList = std.ArrayList;
const StringArrayHashMap = std.StringArrayHashMapUnmanaged;

const HTMLua = @import("htmlua");

const Config = struct {
    component_path: []const u8,
    template_path: []const u8,
};

var component_dir: Io.Dir = undefined;

pub fn main(init: std.process.Init) !void {
    const io = init.io;

    var arena = std.heap.ArenaAllocator.init(init.gpa);
    defer arena.deinit();
    const alloc = arena.allocator();

    var args = init.minimal.args.iterate();

    const cwd = std.Io.Dir.cwd();

    if (!args.skip()) unreachable;
    const config_file_path = args.next().?;
    const config_buf = try cwd.readFileAllocOptions(io, config_file_path, alloc, .unlimited, std.mem.Alignment.@"1", '\x00');
    const config = try std.zon.parse.fromSliceAlloc(
        Config,
        alloc,
        config_buf,
        null,
        .{},
    );

    component_dir = try cwd.openDir(io, config.component_path, .{});
    const template_dir = try cwd.openDir(io, config.template_path, .{});

    var state: HTMLua.Parser = .{
        .tags = &[_]HTMLua.ParserTag{
            HTMLua.tags.Include,
            HTMLua.tags.Export,
            HTMLua.tags.Use,
            HTMLua.tags.Syntax,
        },
        .tag_stack = try ArrayList([]const u8).initCapacity(alloc, 256),
        .exported_parts = try StringArrayHashMap([]const u8).init(alloc, &[0][]const u8{}, &[0][]const u8{}),
        .component_resolver = struct {
            pub fn resolve(arena_inner: *ArenaAllocator, io_inner: Io, path: []const u8) anyerror!*Io.Reader {
                const read_buff = try arena_inner.allocator().alloc(u8, 16384);
                const clean_path = try HTMLua.utils.sanitizePath(arena_inner.allocator(), path);
                var file = try component_dir.openFile(io_inner, clean_path, .{ .mode = .read_only });
                const reader = try arena_inner.allocator().create(File.Reader);
                reader.* = file.reader(io_inner, read_buff);
                return &reader.interface;
            }
        }.resolve,
    };

    const request_path = init.environ_map.get("PATH_INFO").?;
    const clean_request_path = try HTMLua.utils.sanitizePath(alloc, request_path);
    const file = try template_dir.openFile(io, clean_request_path, .{ .mode = .read_only });
    defer file.close(io);

    const in_buf = try alloc.alloc(u8, 16384);
    var in = file.reader(io, in_buf);

    const out_buf = try alloc.alloc(u8, 16384);
    var out = File.stdout().writer(io, out_buf);

    try out.interface.writeAll("Content-type: text/html\n");
    try HTMLua.parseTemplate(&arena, io, &state, &in.interface, &out.interface);

    try out.flush();
}
