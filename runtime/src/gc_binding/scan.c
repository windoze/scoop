/* Exact descriptor scans, optionally restricted to a half-open address range. */
#include "scan.h"

static void scan_single_reference_array(uintptr_t start, uint64_t stride, uint64_t count,
                                         ScoopGcSlotVisitor visitor, void *context,
                                         uintptr_t begin, uintptr_t end) {
    if (end <= start) {
        return;
    }
    /* Round relative slot indices up without adding stride near UINTPTR_MAX. */
    uint64_t first = begin > start ? (begin - start - 1) / stride + 1 : 0;
    uint64_t last = (end - start - 1) / stride + 1;
    if (last > count) {
        last = count;
    }
    for (uint64_t index = first; index < last; index++) {
        visitor((void **)(start + index * stride), context);
    }
}

void scoop_gc_scan_descriptor(void *base, const uint64_t *table, ScoopGcSlotVisitor visitor,
                              void *context, uintptr_t begin, uintptr_t end) {
    if (table == NULL) {
        return;
    }
    if (table[0] == SCOOP_REFS_ARRAY) {
        uint64_t stride = table[3];
        const uint64_t *element_scan = (const uint64_t *)(uintptr_t)table[4];
        if (stride == 0 || element_scan == NULL) {
            scoop_gc_roots_fatal("array scan has an invalid element program");
        }
        uint64_t count = *(const uint64_t *)((char *)base + table[1]);
        char *elements = (char *)base + table[2];
        uintptr_t start = (uintptr_t)elements;
        if (count == 0 || end <= start) {
            return;
        }
        if (element_scan[0] == 1) {
            scan_single_reference_array(start + element_scan[1], stride, count, visitor, context,
                                         begin, end);
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
            scoop_gc_scan_descriptor(elements + index * stride, element_scan, visitor, context,
                                     begin, end);
        }
        return;
    }
    if (table[0] == SCOOP_REFS_SEQUENCE) {
        for (uint64_t index = 0; index < table[1]; index++) {
            scoop_gc_scan_descriptor(base, (const uint64_t *)(uintptr_t)table[2 + index], visitor,
                                     context, begin, end);
        }
        return;
    }
    scoop_gc_scan_references(base, table, visitor, context, begin, end, true);
}
