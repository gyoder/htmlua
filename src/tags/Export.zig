const std = @import("std");
const Io = std.Io;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const SplitIterator = std.mem.SplitIterator;
const DelimiterType = std.mem.DelimiterType;

const parser = @import("../root.zig");
const Parser = parser.Parser;

pub const ExportTagError = error{
    NoPartAttribute,
};

pub const Export: parser.ParserTag = .{
    .name = "export",
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
    _ = writer;

    const raw_part_attr: []const u8 = while (attrs.next()) |attr| {
        var it = std.mem.splitScalar(u8, attr, '=');
        if (std.mem.eql(u8, it.first(), "part")) {
            break it.next() orelse return ExportTagError.NoPartAttribute;
        }
    } else return ExportTagError.NoPartAttribute;

    const part = try arena.allocator().dupe(u8, raw_part_attr);

    var part_buffer = try Io.Writer.Allocating.initCapacity(arena.allocator(), 1024);

    try state.tag_stack.append(arena.allocator(), "export");
    try parser.parseTemplate(
        arena,
        io,
        state,
        reader,
        &part_buffer.writer,
    );
    _ = state.tag_stack.pop();

    try state.exported_parts.put(arena.allocator(), part, try part_buffer.toOwnedSlice());
}
