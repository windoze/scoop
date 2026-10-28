/* Exact descriptor scans, optionally restricted to a half-open address range. */
#include "scan.h"

void scoop_gc_scan_descriptor(void *base, const uint64_t *table, ScoopGcSlotVisitor visitor,
                              void *context, uintptr_t begin, uintptr_t end) {
    scoop_gc_scan_range(base, table, visitor, context, begin, end);
}

void scoop_gc_scan_composite(void *base, const uint64_t *table, ScoopGcSlotVisitor visitor,
                             void *context, uintptr_t begin, uintptr_t end) {
    if (table[0] == SCOOP_REFS_ARRAY) {
        uint64_t stride = table[3];
        const uint64_t *element_scan = (const uint64_t *)(uintptr_t)table[4];
        uint64_t count = *(const uint64_t *)((char *)base + table[1]);
        char *elements = (char *)base + table[2];
        uintptr_t start = (uintptr_t)elements;
        if (count == 0 || end <= start) {
            return;
        }
        uint64_t first = begin > start ? (begin - start) / stride : 0;
        uint64_t last = (end - 1 - start) / stride + 1;
        if (last > count) {
            last = count;
        }
        if (element_scan[0] < SCOOP_REFS_SEQUENCE) {
            for (uint64_t index = first; index < last; index++) {
                scoop_gc_scan_references(elements + index * stride, element_scan, visitor,
                                          context, begin, end, true);
            }
            return;
        }
        for (uint64_t index = first; index < last; index++) {
            scoop_gc_scan_range(elements + index * stride, element_scan, visitor, context,
                                begin, end);
        }
        return;
    }
    for (uint64_t index = 0; index < table[1]; index++) {
        scoop_gc_scan_range(base, (const uint64_t *)(uintptr_t)table[2 + index], visitor,
                            context, begin, end);
    }
}
