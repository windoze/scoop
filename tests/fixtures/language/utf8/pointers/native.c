#define _DEFAULT_SOURCE
#include <assert.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <stdint.h>

const uint8_t *native_bytes(int32_t kind) {
    static const uint8_t valid[] = {65, 0xe7, 0x95, 0x8c, 0, 66};
    static const uint8_t invalid[] = {65, 0xe1, 0x80};
    return kind == 0 ? valid : invalid;
}

static const struct {
    uint8_t bytes[4];
    uint8_t length;
} guarded[] = {
    {{0}, 1}, {{0x7f}, 1}, {{0xc2, 0x80}, 2}, {{0xdf, 0xbf}, 2},
    {{0xe0, 0xa0, 0x80}, 3}, {{0xed, 0x9f, 0xbf}, 3}, {{0xee, 0x80, 0x80}, 3},
    {{0xef, 0xbf, 0xbd}, 3}, {{0xf0, 0x90, 0x80, 0x80}, 4}, {{0xf4, 0x8f, 0xbf, 0xbf}, 4},
    {{0x80, 0x80}, 2}, {{0xc0, 0xaf}, 2}, {{0xe0, 0x9f, 0xbf}, 3},
    {{0xed, 0xa0, 0x80}, 3}, {{0xf0, 0x8f, 0xbf, 0xbf}, 4},
    {{0xf4, 0x90, 0x80, 0x80}, 4}, {{0xf5, 0x80, 0x80, 0x80}, 4},
    {{0xff}, 1}, {{0xe1, 0x80, 0x41}, 3}, {{0xf1, 0x80, 0x80, 0x41}, 4},
    {{0xc2}, 1}, {{0xe1, 0x80}, 2}, {{0xf0, 0x90, 0x80}, 3},
};

int64_t native_guarded_length(int32_t kind) { return guarded[kind].length; }

const uint8_t *native_guarded_bytes(int32_t kind) {
    static uint8_t *storage;
    static size_t page;
    assert(kind >= 0 && (size_t)kind < sizeof guarded / sizeof guarded[0]);
    if (storage == NULL) {
        long size = sysconf(_SC_PAGESIZE);
        assert(size > 0);
        page = (size_t)size;
        storage = mmap(NULL, page * 2, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
        assert(storage != MAP_FAILED);
        assert(mprotect(storage + page, page, PROT_NONE) == 0);
    }
    uint8_t *bytes = storage + page - guarded[kind].length;
    memcpy(bytes, guarded[kind].bytes, guarded[kind].length);
    return bytes;
}
