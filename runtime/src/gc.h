#ifndef SCOOP_RUNTIME_GC_H
#define SCOOP_RUNTIME_GC_H

#include <string.h>

#include "scoop_rt.h"

/* Runtime operations. Allocation may collect: the caller has already published
 * its managed entry and live roots, and td is already registered. No private
 * heap metadata escapes this API. */
void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size);
bool scoop_gc_is_published_object(const void *object);
void scoop_gc_check_array_clone_source(const void *object, const ScoopTypeDescriptor *expected);
void scoop_gc_check_unbox(const void *object, const ScoopTypeDescriptor *expected,
                          uint32_t storage_kind);
void scoop_gc_shutdown(void);
void scoop_gc_report_metrics(void);
void scoop_gc_adjust_pressure(int64_t delta);

/* Complete NoGC heap copy, retaining the existing single range barrier. */
static inline void scoop_gc_copy_value(void *destination, const void *source, size_t bytes,
                                       const uint64_t *scan) {
    memcpy(destination, source, bytes);
    if (scan != NULL) {
        scoop_rt_gc_write_barrier(destination, bytes);
    }
}

#endif
