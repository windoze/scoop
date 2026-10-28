#ifndef SCOOP_GC_BINDING_SCAN_H
#define SCOOP_GC_BINDING_SCAN_H

#include "../gc_binding.h"
#include "scoop_rt.h"

/* Immutable offsets shared by full-object and range-restricted scans. */
static inline void scoop_gc_scan_references(char *base, const uint64_t *table,
                                            ScoopGcSlotVisitor visitor, void *context,
                                            uintptr_t begin, uintptr_t end, bool bounded) {
    uint64_t count = table[0];
    for (uint64_t index = 0; index < count; index++) {
        void **slot = (void **)(base + table[1 + index]);
        if (!bounded || ((uintptr_t)slot >= begin && (uintptr_t)slot < end)) {
            visitor(slot, context);
        }
    }
}

/* Only multi-reference arrays and recursive compositions reach this shared body. */
void scoop_gc_scan_composite(void *base, const uint64_t *table, ScoopGcSlotVisitor visitor,
                             void *context, uintptr_t begin, uintptr_t end);

static inline void scoop_gc_scan_single_reference_array(uintptr_t start, uint64_t stride,
                                                         uint64_t count,
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

/* Known visitors share the same range calculation as the generic descriptor entry. */
static inline void scoop_gc_scan_range(void *base, const uint64_t *table,
                                       ScoopGcSlotVisitor visitor, void *context,
                                       uintptr_t begin, uintptr_t end) {
    if (table == NULL) {
        return;
    }
    if (table[0] < SCOOP_REFS_SEQUENCE) {
        scoop_gc_scan_references(base, table, visitor, context, begin, end, true);
        return;
    }
    if (table[0] == SCOOP_REFS_ARRAY) {
        uint64_t stride = table[3];
        const uint64_t *element_scan = (const uint64_t *)(uintptr_t)table[4];
        if (stride == 0 || element_scan == NULL) {
            scoop_gc_roots_fatal("array scan has an invalid element program");
        }
        if (element_scan[0] == 1) {
            uint64_t count = *(const uint64_t *)((char *)base + table[1]);
            uintptr_t start = (uintptr_t)base + table[2] + element_scan[1];
            scoop_gc_scan_single_reference_array(start, stride, count, visitor, context, begin, end);
            return;
        }
    }
    scoop_gc_scan_composite(base, table, visitor, context, begin, end);
}

/* Keep the flat case visible to callers with a statically known visitor. */
static inline void scoop_gc_scan_object(void *base, const uint64_t *table,
                                        ScoopGcSlotVisitor visitor, void *context) {
    if (table == NULL) {
        return;
    }
    if (table[0] < SCOOP_REFS_SEQUENCE) {
        scoop_gc_scan_references(base, table, visitor, context, 0, UINTPTR_MAX, false);
    } else {
        scoop_gc_scan_descriptor(base, table, visitor, context, 0, UINTPTR_MAX);
    }
}

#endif
