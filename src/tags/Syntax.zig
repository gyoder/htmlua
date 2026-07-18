const std = @import("std");
const Io = std.Io;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const SplitIterator = std.mem.SplitIterator;
const DelimiterType = std.mem.DelimiterType;

const ezts = @import("easy-zig-tree-sitter");
const ts = @import("tree-sitter");

const parser = @import("../root.zig");
const Parser = parser.Parser;

pub const SyntaxTagError = error{InvalidLanguage};
pub const Syntax: parser.ParserTag = .{
    .name = "syntax",
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
    _ = state;
    _ = io;

    const raw_lang_attr: []const u8 = while (attrs.next()) |attr| {
        var it = std.mem.splitScalar(u8, attr, '=');
        if (std.mem.eql(u8, it.first(), "lang")) {
            break it.next() orelse "";
        }
    } else "";

    const lang_key = try arena.allocator().dupe(u8, raw_lang_attr);

    _ = try reader.takeDelimiterInclusive('\n');
    const first_line = try reader.peekDelimiterInclusive('\n');
    const whitespace_chr = for (first_line, 0..) |chr, idx| {
        if (!(chr == ' ' or chr == '\t')) break idx;
    } else 0;

    var code = try std.ArrayList(u8).initCapacity(arena.allocator(), 1024);

    while (true) {
        const line = try reader.takeDelimiterInclusive('\n');
        if (std.mem.indexOf(u8, line, "</syntax>")) |idx| {
            try code.appendSlice(arena.allocator(), line[0..idx]);
            // TODO: tags at the end will break the parser
            break;
        }
        const whitespace_lem = @min(line.len, whitespace_chr);

        if (std.mem.countScalar(u8, line[0..whitespace_lem], ' ') + std.mem.countScalar(u8, line[0..whitespace_lem], '\t') < whitespace_lem) {
            try code.appendSlice(arena.allocator(), line);
        } else {
            try code.appendSlice(arena.allocator(), line[whitespace_lem..line.len]);
        }
    }

    // Alignment issues?
    // ts.setAllocator(arena.allocator());
    // TODO: TS Thread Mutex

    // TODO: null handeling
    var hlp = HLParser.create(lang_key, code.items) orelse {
        std.log.warn("Failed to make Parser for {s}", .{lang_key});
        return;
    };
    defer hlp.destroy();
    var next_hl = hlp.nextHL();

    var end_stack = try std.ArrayList(u32).initCapacity(arena.allocator(), 1024);
    var code_idx: u32 = 0;

    try writer.writeAll("<pre class=\"hl\"><code>");

    while (code_idx != code.items.len) {
        var next_idx: u32 = @intCast(code.items.len);
        if (end_stack.getLastOrNull()) |last| {
            if (last == code_idx) {
                try writer.writeAll("</span>");
                _ = end_stack.pop();
                continue;
            } else if (last < next_idx) {
                next_idx = last;
            }
        }
        if (next_hl) |hl| {
            if (hl.start == code_idx) {
                try writer.writeAll("<span class=\"hl");
                // no '.' in css class names
                var it = std.mem.splitScalar(u8, hl.name, '.');
                while (it.next()) |n| {
                    try writer.writeByte('-');
                    try writer.writeAll(n);
                }
                try writer.writeAll("\">");
                try end_stack.append(arena.allocator(), hl.end);
                next_hl = hlp.nextHL();
                continue;
            } else if (hl.start < next_idx) {
                next_idx = hl.start;
            }
        }
        try writer.writeAll(code.items[code_idx..next_idx]);
        code_idx = next_idx;
    }

    try writer.writeAll("</code></pre>");
}

const HLGroup = struct {
    name: []const u8,
    start: u32,
    end: u32,
};

const HLParser = struct {
    lang: *ts.Language,
    tree: *ts.Tree,
    query: *ts.Query,
    cursor: *ts.QueryCursor,
    fn create(lang_key: []const u8, code: []const u8) ?HLParser {
        const lang = ezts.getLang(lang_key) orelse return null;
        const query_string = ezts.getQuery(lang_key, "highlights") orelse return null;

        const ps = ts.Parser.create();

        ps.setLanguage(lang) catch return null;
        const tree = ps.parseString(code, null) orelse return null;

        var error_offset: u32 = 0;
        const query = ts.Query.create(lang, query_string, &error_offset) catch return null;

        const cursor = ts.QueryCursor.create();
        cursor.exec(query, tree.rootNode());

        return .{
            .lang = lang,
            .tree = tree,
            .query = query,
            .cursor = cursor,
        };
    }

    fn destroy(self: *HLParser) void {
        self.cursor.destroy();
        self.query.destroy();
        self.tree.destroy();
        self.lang.destroy();
    }

    fn nextHL(self: *HLParser) ?HLGroup {
        if (self.cursor.nextCapture()) |data| {
            const idx, const match = data;
            const c = match.captures[idx];

            return .{
                .start = c.node.startByte(),
                .end = c.node.endByte(),
                .name = self.query.captureNameForId(c.index) orelse return null,
            };
        } else return null;
    }
};
