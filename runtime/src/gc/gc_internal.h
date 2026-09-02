#ifndef SCOOP_GC_INTERNAL_H
#define SCOOP_GC_INTERNAL_H

#include <stdbool.h>
#include <stdint.h>

#include "scoop_rt.h"
#include "stackmap.h"

typedef void (*ScoopGcSlotVisitor)(void **slot, void *context);
typedef void (*ScoopGcExternalObjectVisitor)(const void *object,
                                             void *context);
typedef void (*ScoopGcRegionVisitor)(void *base, const uint64_t *scan,
                                    void *context);

typedef struct ScoopGcRootVisitor {
    ScoopGcSlotVisitor visit_slot;
    ScoopGcExternalObjectVisitor visit_external_object;
    ScoopGcRegionVisitor visit_region;
    void *context;
} ScoopGcRootVisitor;

/* Heap metadata operations used by the pin registry. The `_locked` query
 * requires the caller to hold the heap lock acquired through this API. */
void scoop_gc_heap_lock(void);
void scoop_gc_heap_unlock(void);
bool scoop_gc_is_object_start_locked(const void *object);
bool scoop_gc_mark_object_locked(const void *object);
void scoop_gc_heap_begin_collection_locked(void);
void scoop_gc_heap_finish_collection_locked(uint64_t live_objects);

/* Collection holds this lock while it visits and rewrites root slots;
 * mutator root APIs use the same lock. */
void scoop_gc_roots_lock(void);
void scoop_gc_roots_unlock(void);
void scoop_gc_register_image_roots(
    const ScoopManagedGlobalDescriptor *managed_globals,
    uint64_t managed_global_count,
    const ScoopImmortalObjectDescriptor *immortal_objects,
    uint64_t immortal_object_count);
bool scoop_gc_is_immortal_object_locked(const void *object);
void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor);

/* Immutable loaded-image stack-map index. Initialization parses and validates
 * every record through the selected target profile before managed code runs. */
void scoop_gc_stackmaps_init(void);
const ScoopStackMapRecord *scoop_gc_stackmap_lookup(uintptr_t return_pc);

#endif
