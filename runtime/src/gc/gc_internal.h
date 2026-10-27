#ifndef SCOOP_GC_INTERNAL_H
#define SCOOP_GC_INTERNAL_H

#include <stdbool.h>
#include <stdint.h>

#include "../gc.h"
#include "../gc_binding.h"
#include "../generated_entries.h"
#include "stress.h"

typedef void (*ScoopGcHeapObjectVisitor)(void *object, void *context);
typedef void (*ScoopGcLiveEntryVisitor)(void **entry, void *context);

typedef struct ScoopGcSurvivors {
    size_t capacity;
    uint64_t live_bytes;
    uint64_t objects;
} ScoopGcSurvivors;

/* Arena-external side metadata and moving-heap operations. Every `_locked`
 * operation requires the heap lock acquired through this API. */
void scoop_gc_heap_init(void);
#if SCOOP_ENABLE_GC_STRESS
bool scoop_gc_stress_move_enabled(void);
#else
#define scoop_gc_stress_move_enabled() false
#endif
void scoop_gc_heap_lock(void);
void scoop_gc_heap_unlock(void);
bool scoop_gc_is_object_start_locked(const void *object);
size_t scoop_gc_object_size_locked(const void *object);
bool scoop_gc_update_pin_locked(const void *object, bool pinned);
bool scoop_gc_is_young_object_locked(const void *object);
size_t scoop_gc_heap_begin_collection_locked(bool minor);
bool scoop_gc_heap_plan_moving_locked(bool minor, ScoopGcSurvivors *survivors);
#if SCOOP_ENABLE_GC_STRESS
void scoop_gc_heap_verify_stress_moved_locked(void);
#endif
#if SCOOP_GC_TRACK_RELOCATION
void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor, void *context);
#endif
void scoop_gc_heap_finish_collection_locked(uint64_t live_objects, bool minor,
                                             const ScoopGcSurvivors *survivors);

void scoop_gc_visit_handles_locked(ScoopGcRootVisitor visitor);
void scoop_gc_set_pin_frames_locked(bool pinned_now);

/* True when this request performed a full collection, false when it joined
 * another collector. A joined collection may have been minor or a mark slice. */
bool scoop_gc_collect_internal(void);
/* Wait for a full completed after this explicit request, coalescing concurrent fulls. */
void scoop_gc_collect_explicit_internal(void);
void scoop_gc_collect_minor_internal(void);
void scoop_gc_collect_incremental_internal(bool minor);

/* One parameterized marker, with a retained live set until reference update ends. */
void scoop_gc_mark_begin(bool minor, size_t block_count);
void scoop_gc_mark_roots(void);
void scoop_gc_mark_remembered(void);
bool scoop_gc_mark_dirty_groups(bool retain_remembered, bool first);
void scoop_gc_mark_restore_remembered(void);
void scoop_gc_mark_publish(void);
bool scoop_gc_mark_step(bool finish);
void scoop_gc_mark_resume(size_t retained_blocks, size_t total_blocks);
uint64_t scoop_gc_mark_complete(bool minor);
uint64_t scoop_gc_mark_finish(bool minor);
void scoop_gc_mark_visit_live(ScoopGcLiveEntryVisitor visitor, void *context);
void scoop_gc_mark_dispose(void);
void scoop_gc_update_live(bool minor, ScoopGcSlotVisitor visitor,
                          ScoopGcLiveEntryVisitor serial_visitor, void *context);
uint64_t scoop_gc_monotonic_ns(void);
uint64_t scoop_gc_thread_cpu_ns(void);

typedef void (*ScoopGcObjectRangeVisitor)(void *object, uintptr_t begin, uintptr_t end,
                                          void *context);
void scoop_gc_scan_remembered(ScoopGcObjectRangeVisitor visitor, void *context, bool count_cards);
struct ScoopGcCardGroup;
uint64_t scoop_gc_scan_dirty_group(struct ScoopGcCardGroup *group,
                                   ScoopGcObjectRangeVisitor visitor, void *context, bool only_old);

#endif
