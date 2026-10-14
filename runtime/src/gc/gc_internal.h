#ifndef SCOOP_GC_INTERNAL_H
#define SCOOP_GC_INTERNAL_H

#include <stdbool.h>
#include <stdint.h>

#include "../gc.h"
#include "../generated_entries.h"
#include "../gc_binding.h"

typedef void (*ScoopGcHeapObjectVisitor)(void *object, void *context);

/* Arena-external side metadata and moving-heap operations. Every `_locked`
 * operation requires the heap lock acquired through this API. */
void scoop_gc_heap_init(void);
bool scoop_gc_stress_move_enabled(void);
void scoop_gc_heap_lock(void);
void scoop_gc_heap_unlock(void);
bool scoop_gc_is_object_start_locked(const void *object);
size_t scoop_gc_object_size_locked(const void *object);
bool scoop_gc_update_pin_locked(const void *object, bool pinned);
bool scoop_gc_is_young_object_locked(const void *object);
size_t scoop_gc_heap_begin_collection_locked(bool minor);
bool scoop_gc_heap_plan_moving_locked(bool minor);
void scoop_gc_heap_verify_stress_moved_locked(void);
void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor, void *context);
void scoop_gc_heap_finish_collection_locked(uint64_t live_objects, bool minor);

void scoop_gc_visit_handles_locked(ScoopGcRootVisitor visitor);
void scoop_gc_set_pin_frames_locked(bool pinned_now);

/* True when this request performed a full collection, false when it joined
 * another collector. A joined collection may have been minor. */
bool scoop_gc_collect_internal(void);
void scoop_gc_collect_minor_internal(void);

/* One parameterized marker, with a retained live set until reference update ends. */
void scoop_gc_mark_begin(bool minor, size_t block_count);
void scoop_gc_mark_roots(void);
void scoop_gc_mark_remembered(void);
uint64_t scoop_gc_mark_finish(bool minor);
void scoop_gc_mark_visit_live(ScoopGcHeapObjectVisitor visitor, void *context);
void scoop_gc_mark_dispose(void);
uint64_t scoop_gc_monotonic_ns(void);
uint64_t scoop_gc_thread_cpu_ns(void);

typedef void (*ScoopGcObjectRangeVisitor)(void *object, uintptr_t begin, uintptr_t end,
                                          void *context);
void scoop_gc_scan_remembered(ScoopGcObjectRangeVisitor visitor, void *context, bool count_cards);

#endif
