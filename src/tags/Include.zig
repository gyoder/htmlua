const std = @import("std");
const Io = std.Io;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const SplitIterator = std.mem.SplitIterator;
const DelimiterType = std.mem.DelimiterType;

const parser = @import("../root.zig");
const Parser = parser.Parser;

pub const IncludeTagError = error{
    NoPathAttribute,
    PathOpenFailure,
};

pub const Include: parser.ParserTag = .{
    .name = "include",
    .parse_fn = &parse,
};

fn parse(
    arena: *ArenaAllocator,
    io: Io,
    state: *Parser,
    reader: *Io.Reader,
    writer: *Io.Writer,
    attrs: *SplitIterator(u8, DelimiterType.scalar),
) anyerror!void {
    const raw_path_attr: []const u8 = while (attrs.next()) |attr| {
        var it = std.mem.splitScalar(u8, attr, '=');
        if (std.mem.eql(u8, it.first(), "path")) {
            const raw_path = it.next() orelse return IncludeTagError.NoPathAttribute;
            if (raw_path.len == 0) return IncludeTagError.NoPathAttribute;
            if ((raw_path[0] == '"' and raw_path[raw_path.len - 1] == '"') or
                (raw_path[0] == '\'' and raw_path[raw_path.len - 1] == '\''))
            {
                if (raw_path.len <= 2) return IncludeTagError.NoPathAttribute;
                break raw_path[1 .. raw_path.len - 1];
            } else {
                break raw_path;
            }
        }
    } else return IncludeTagError.NoPathAttribute;

    const path = try arena.allocator().dupe(u8, raw_path_attr);

    // parse through to gather <export> tags
    try state.tag_stack.append(arena.allocator(), "include");
    try parser.parseTemplate(arena, io, state, reader, writer);
    _ = state.tag_stack.pop();

    const included_reader = state.component_resolver(arena, io, path) catch {
        std.log.err("Include Tag: failed to open file {s}", .{path});
        return IncludeTagError.PathOpenFailure;
    };

    try parser.parseTemplate(arena, io, state, included_reader, writer);
}
