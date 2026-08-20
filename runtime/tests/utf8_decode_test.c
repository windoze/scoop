#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include "../src/utf8.h"

static void all_scalars_and_truncated_prefixes(void) {
    for (uint32_t scalar = 0; scalar <= 0x10ffff; ++scalar) {
        if (scalar >= 0xd800 && scalar <= 0xdfff) {
            continue;
        }
        unsigned char bytes[4];
        uint64_t length = scoop_utf8_encode(bytes, scalar);
        ScoopUtf8Step step = scoop_utf8_next(bytes, length);
        assert(step.valid && step.scalar == scalar && step.consumed == length);
        for (uint64_t prefix = 1; prefix < length; ++prefix) {
            step = scoop_utf8_next(bytes, prefix);
            assert(!step.valid && step.scalar == 0xfffd && step.consumed == prefix);
        }
    }
}

static void maximal_subparts_at_memory_boundary(void) {
    const struct {
        unsigned char bytes[4];
        uint8_t length;
        uint8_t consumed;
        bool valid;
        uint32_t scalar;
    } cases[] = {
        {{0}, 1, 1, true, 0},
        {{0x7f}, 1, 1, true, 0x7f},
        {{0xc2, 0x80}, 2, 2, true, 0x80},
        {{0xdf, 0xbf}, 2, 2, true, 0x7ff},
        {{0xe0, 0xa0, 0x80}, 3, 3, true, 0x800},
        {{0xed, 0x9f, 0xbf}, 3, 3, true, 0xd7ff},
        {{0xee, 0x80, 0x80}, 3, 3, true, 0xe000},
        {{0xef, 0xbf, 0xbd}, 3, 3, true, 0xfffd},
        {{0xf0, 0x90, 0x80, 0x80}, 4, 4, true, 0x10000},
        {{0xf4, 0x8f, 0xbf, 0xbf}, 4, 4, true, 0x10ffff},
        {{0x80, 0x80}, 2, 1, false, 0xfffd},
        {{0xc0, 0xaf}, 2, 1, false, 0xfffd},
        {{0xe0, 0x9f, 0xbf}, 3, 1, false, 0xfffd},
        {{0xed, 0xa0, 0x80}, 3, 1, false, 0xfffd},
        {{0xf0, 0x8f, 0xbf, 0xbf}, 4, 1, false, 0xfffd},
        {{0xf4, 0x90, 0x80, 0x80}, 4, 1, false, 0xfffd},
        {{0xf5, 0x80, 0x80, 0x80}, 4, 1, false, 0xfffd},
        {{0xff}, 1, 1, false, 0xfffd},
        {{0xe1, 0x80, 0x41}, 3, 2, false, 0xfffd},
        {{0xf1, 0x80, 0x80, 0x41}, 4, 3, false, 0xfffd},
        {{0xc2}, 1, 1, false, 0xfffd},
        {{0xe1, 0x80}, 2, 2, false, 0xfffd},
        {{0xf0, 0x90, 0x80}, 3, 3, false, 0xfffd},
    };
    long raw_page_size = sysconf(_SC_PAGESIZE);
    assert(raw_page_size > 0);
    size_t page = (size_t)raw_page_size;
    unsigned char *storage =
        mmap(NULL, page * 2, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    assert(storage != MAP_FAILED);
    assert(mprotect(storage + page, page, PROT_NONE) == 0);
    for (size_t index = 0; index < sizeof cases / sizeof cases[0]; ++index) {
        const uint8_t length = cases[index].length;
        unsigned char *bytes = storage + page - length;
        memcpy(bytes, cases[index].bytes, length);
        ScoopUtf8Step step = scoop_utf8_next(bytes, length);
        assert(step.valid == cases[index].valid && step.scalar == cases[index].scalar &&
               step.consumed == cases[index].consumed);
    }
    assert(munmap(storage, page * 2) == 0);
}

int main(void) {
    all_scalars_and_truncated_prefixes();
    maximal_subparts_at_memory_boundary();
    puts("UTF-8 scalars, maximal subparts and bounded reads passed");
    return 0;
}
