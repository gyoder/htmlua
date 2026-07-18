const std = @import("std");
const Io = std.Io;
const File = Io.File;
const Allocator = std.mem.Allocator;
const ArenaAllocator = std.heap.ArenaAllocator;
const ArrayList = std.ArrayList;
const StringArrayHashMap = std.StringArrayHashMapUnmanaged;

pub fn sanitizePath(alloc: Allocator, path: []const u8) ![]const u8 {
    const clean = try alloc.alloc(u8, path.len);
    var list = ArrayList(u8).initBuffer(clean);
    var pit = std.mem.splitScalar(u8, path, '/');
    while (pit.next()) |part| {
        if (part.len == 0) continue;
        if (std.mem.eql(u8, part, "..")) continue;
        // This should never actually allocate
        try list.appendSlice(alloc, part);
        try list.append(alloc, '/');
    }
    const new_len = list.items.len - 1;
    _ = alloc.resize(clean, new_len);
    return clean[0..new_len];
}
