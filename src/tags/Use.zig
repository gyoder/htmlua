const std = @import("std");
const Io = std.Io;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const SplitIterator = std.mem.SplitIterator;
const DelimiterType = std.mem.DelimiterType;

const parser = @import("../root.zig");
const Parser = parser.Parser;

pub const UseTagError = error{ NoPartAttribute, NoPartExported };

pub const Use: parser.ParserTag = .{
    .name = "use",
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
    _ = io;

    const raw_part_attr: []const u8 = while (attrs.next()) |attr| {
        var it = std.mem.splitScalar(u8, attr, '=');
        if (std.mem.eql(u8, it.first(), "part")) {
            break it.next() orelse return UseTagError.NoPartAttribute;
        }
    } else return UseTagError.NoPartAttribute;

    const part = try arena.allocator().dupe(u8, raw_part_attr);

    const part_text = state.exported_parts.get(part) orelse return UseTagError.NoPartExported;

    var part_reader = Io.Reader.fixed(part_text);

    _ = try part_reader.streamRemaining(writer);

    while (true) {
        _ = try reader.discardDelimiterInclusive('<');
        const inside = try reader.takeDelimiterInclusive('>');
        if (std.mem.eql(u8, "/use>", inside)) return;
    }
}
