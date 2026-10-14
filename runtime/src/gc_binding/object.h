#ifndef SCOOP_GC_BINDING_OBJECT_H
#define SCOOP_GC_BINDING_OBJECT_H

#include "../value_shape.h"

/* Immutable views of already validated Scoop metadata. Keep trivial accessors
 * inline: separating representation must not add per-object runtime calls. */
static inline const uint64_t *scoop_gc_object_scan(const void *object) {
    return ((const ScoopObjectHeader *)object)->td->object_scan;
}

static inline void scoop_gc_validate_object(const void *object, size_t size) {
    scoop_shape_validate_object(object, size);
}

static inline bool scoop_gc_scan_is_array(const uint64_t *scan) {
    return scan != NULL && scan[0] == SCOOP_REFS_ARRAY;
}

typedef struct ScoopGcArrayScan {
    uint64_t count;
    uintptr_t elements;
    uint64_t stride;
} ScoopGcArrayScan;

/* The caller has selected the array branch after normal object validation. */
static inline ScoopGcArrayScan scoop_gc_array_scan(const void *object, const uint64_t *scan) {
    return (ScoopGcArrayScan){
        .count = *(const uint64_t *)((const char *)object + scan[1]),
        .elements = (uintptr_t)object + scan[2],
        .stride = scan[3],
    };
}

#endif
