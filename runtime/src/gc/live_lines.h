#ifndef SCOOP_GC_LIVE_LINES_H
#define SCOOP_GC_LIVE_LINES_H

#include "heap_internal.h"

static inline __attribute__((always_inline)) void
scoop_gc_record_live_lines(uint64_t *lines, size_t word, size_t size) {
    size_t first = word * sizeof(uint64_t) / GC_LINE_SIZE;
    size_t last = (word * sizeof(uint64_t) + size - 1) / GC_LINE_SIZE;
    uint64_t head = UINT64_MAX << (first % 64);
    uint64_t tail = UINT64_MAX >> (63 - last % 64);
    if (first / 64 == last / 64) {
        lines[first / 64] |= head & tail;
        return;
    }
    lines[first / 64] |= head;
    for (size_t index = first / 64 + 1; index < last / 64; index++) {
        lines[index] = UINT64_MAX;
    }
    lines[last / 64] |= tail;
}

#endif
