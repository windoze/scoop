#ifndef SCOOP_GC_OLD_SLOTS_H
#define SCOOP_GC_OLD_SLOTS_H

#include "heap_internal.h"

typedef struct ScoopGcOldSlotPlan {
    ScoopGcOldSlots *current[GC_OLD_SLOT_CLASSES];
} ScoopGcOldSlotPlan;

static inline bool scoop_heap_uses_old_slots(void) {
    return scoop_gc_heap_state.old_slots_level != 0 && scoop_heap_adaptive_nursery_enabled();
}

static inline bool scoop_heap_promotes_slot_blocks(void) {
    return scoop_gc_heap_state.old_slots_level >= 2 && scoop_heap_adaptive_nursery_enabled();
}

/* Heap lock held. Nursery objects contribute together at minor completion. */
static inline void scoop_heap_note_old_bytes(size_t bytes) {
    if (scoop_heap_uses_old_slots()) {
        size_t previous = scoop_gc_heap_state.old_pressure_bytes;
        scoop_gc_heap_state.old_pressure_bytes =
            bytes > SIZE_MAX - previous ? SIZE_MAX : previous + bytes;
    }
}

/* The slot cursor is exclusive; parallel planners lock only to claim a block. */
void *scoop_heap_reserve_old_slot(ScoopGcOldSlotPlan *plan, size_t size,
                                 ScoopGcBlockMeta **target, pthread_mutex_t *reservation_lock);
void scoop_heap_rollback_old_slots(void);
void scoop_heap_finish_old_slot_reservations(void);
void scoop_heap_clear_old_slots(void);
/* Complete surviving bytes from before sweep cleared the block statistics. */
void scoop_heap_record_old_slots(ScoopGcBlockMeta *block, size_t live_bytes);
/* Complete mark, dense unpinned nursery block, no retained survivor. */
bool scoop_heap_select_slot_source(ScoopGcBlockMeta *block, size_t *available,
                                   size_t *copyable_bytes, bool selected);

#endif
