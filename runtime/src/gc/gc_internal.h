#ifndef SCOOP_GC_INTERNAL_H
#define SCOOP_GC_INTERNAL_H

#include <stdbool.h>
#include <stdint.h>

#include "../generated_entries.h"
#include "../image/registry.h"
#include "stackmap.h"

typedef void (*ScoopGcSlotVisitor)(void **slot, void *context);
typedef void (*ScoopGcExternalObjectVisitor)(const void *object, void *context);
typedef void (*ScoopGcRegionVisitor)(void *base, const uint64_t *scan, void *context);

typedef struct ScoopGcRootVisitor {
    ScoopGcSlotVisitor visit_slot;
    ScoopGcExternalObjectVisitor visit_external_object;
    ScoopGcRegionVisitor visit_region;
    void *context;
} ScoopGcRootVisitor;

typedef void (*ScoopGcHeapObjectVisitor)(void *object, void *context);

struct ScoopThreadState;

/* Arena-external side metadata and moving-heap operations. Every `_locked`
 * operation requires the heap lock acquired through this API. */
void scoop_gc_heap_init(void);
bool scoop_gc_stress_move_enabled(void);
void scoop_gc_heap_lock(void);
void scoop_gc_heap_unlock(void);
bool scoop_gc_is_object_start_locked(const void *object);
size_t scoop_gc_object_size_locked(const void *object);
bool scoop_gc_update_pin_locked(const void *object, bool pinned);
bool scoop_gc_mark_object_locked(const void *object);
void scoop_gc_heap_begin_collection_locked(void);
void scoop_gc_heap_plan_moving_locked(void);
void scoop_gc_heap_verify_stress_moved_locked(void);
void *scoop_gc_forward_object_locked(void *object);
bool scoop_gc_claim_object_scan_locked(void *object);
bool scoop_gc_object_was_scanned_locked(const void *object);
bool scoop_gc_is_forwarded_old_locked(const void *object);
bool scoop_gc_is_current_live_object_locked(const void *object);
void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor,
                                           void *context);
void scoop_gc_heap_finish_collection_locked(uint64_t live_objects);
void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size);

/* Collection holds this lock while it visits and rewrites root slots;
 * mutator root APIs use the same lock. */
void scoop_gc_roots_lock(void);
void scoop_gc_roots_unlock(void);
_Noreturn void scoop_gc_roots_fatal(const char *message);
bool scoop_gc_is_immortal_object_locked(const void *object);
bool scoop_gc_is_external_object_locked(const void *object);
bool scoop_gc_is_published_object(const void *object);
void scoop_gc_visit_handles_locked(ScoopGcRootVisitor visitor);
void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor);

/* Startup publishes the complete, already resolved loaded-image index. */
void scoop_rt_gc_init(const ScoopImageRegistry *registry);
void scoop_gc_stackmaps_init(const ScoopStackMapIndex *index);
const ScoopStackMapRecord *scoop_gc_stackmap_lookup(uintptr_t return_pc);
void scoop_gc_visit_managed_stack(const struct ScoopThreadState *thread,
                                  ScoopGcRootVisitor visitor);
void scoop_gc_visit_managed_segment(const struct ScoopThreadState *thread,
                                    uintptr_t return_pc, uintptr_t stack_pointer,
                                    uintptr_t frame_pointer, uintptr_t managed_boundary,
                                    ScoopGcRootVisitor visitor);
void scoop_gc_collect_internal(void);

#endif
