const std = @import("std");
const Io = std.Io;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const SplitIterator = std.mem.SplitIterator;
const DelimiterType = std.mem.DelimiterType;
const ArrayList = std.ArrayList;
const StringArrayHashMap = std.StringArrayHashMapUnmanaged;

pub const tags = @import("tags.zig");
pub const utils = @import("utils.zig");

pub const ParserFn =
    fn (
        arena: *ArenaAllocator,
        io: Io,
        state: *Parser,
        reader: *Io.Reader,
        writer: *Io.Writer,
        attrs: *SplitIterator(u8, DelimiterType.scalar),
    ) anyerror!void;

pub const ParserTag = struct {
    name: []const u8,
    parse_fn: *const ParserFn,
};

pub const Parser = struct {
    component_resolver: *const fn (alloc: *ArenaAllocator, io: Io, path: []const u8) anyerror!*Io.Reader,
    exported_parts: StringArrayHashMap([]const u8),
    tags: []const ParserTag,
    tag_stack: ArrayList([]const u8),
};

pub fn parseTemplate(arena: *ArenaAllocator, io: Io, state: *Parser, reader: *Io.Reader, writer: *Io.Writer) !void {
    outer: while (true) {
        _ = reader.streamDelimiter(writer, '<') catch |err| {
            if (err == Io.Reader.Error.EndOfStream) break else return err;
        };
        // consume '<'
        _ = try reader.takeByte();
        if (try reader.peekByte() == '/') {
            // consume '/'
            _ = try reader.takeByte();
            const last_tag = state.tag_stack.getLastOrNull() orelse "";
            // if reached closing tag, send execution back up the stack
            if (std.mem.eql(u8, last_tag, try reader.peekDelimiterExclusive('>'))) {
                // consume closing tag
                _ = try reader.takeDelimiterInclusive('>');
                return;
            } else {
                try writer.writeAll("</");
                continue :outer;
            }
        }
        const inside = reader.takeDelimiterExclusive('>') catch |err| {
            if (err == error.StreamTooLong) {
                try writer.writeByte('<');
                break :outer;
            } else return err;
        };
        var it = std.mem.splitScalar(u8, inside, ' ');
        const tag_name = it.first();
        for (state.tags) |tag| {
            if (std.mem.eql(u8, tag.name, tag_name) == true) {
                // consume '>'
                _ = try reader.takeByte();
                try tag.parse_fn(arena, io, state, reader, writer, &it);
                continue :outer;
            }
        }

        try writer.writeByte('<');
        try writer.writeAll(inside);
    }
}
