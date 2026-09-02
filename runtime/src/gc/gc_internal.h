#ifndef SCOOP_GC_INTERNAL_H
#define SCOOP_GC_INTERNAL_H

#include <stdbool.h>

typedef void (*ScoopGcSlotVisitor)(void **slot, void *context);
typedef void (*ScoopGcExternalObjectVisitor)(const void *object,
                                             void *context);

typedef struct ScoopGcRootVisitor {
    ScoopGcSlotVisitor visit_slot;
    ScoopGcExternalObjectVisitor visit_external_object;
    void *context;
} ScoopGcRootVisitor;

/* Heap metadata operations used by the pin registry. The `_locked` query
 * requires the caller to hold the heap lock acquired through this API. */
void scoop_gc_heap_lock(void);
void scoop_gc_heap_unlock(void);
bool scoop_gc_is_object_start_locked(const void *object);

/* Collection holds this lock while it visits and rewrites root slots;
 * mutator root APIs use the same lock. */
void scoop_gc_roots_lock(void);
void scoop_gc_roots_unlock(void);
void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor);

#endif
