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
