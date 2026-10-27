#ifndef SCOOP_GC_OLD_SLOTS_H
#define SCOOP_GC_OLD_SLOTS_H

#include "heap_internal.h"

typedef struct ScoopGcOldSlotPlan {
    ScoopGcOldSlots *current[GC_OLD_SLOT_CLASSES];
} ScoopGcOldSlotPlan;

static inline bool scoop_heap_uses_old_slots(void) {
    return scoop_gc_heap_state.old_slots_enabled && scoop_heap_adaptive_nursery_enabled();
}

/* Heap lock held. Nursery objects contribute together at minor completion. */
static inline void scoop_heap_note_old_bytes(size_t bytes) {
    if (scoop_heap_uses_old_slots()) {
        size_t previous = scoop_gc_heap_state.old_pressure_bytes;
        scoop_gc_heap_state.old_pressure_bytes =
            bytes > SIZE_MAX - previous ? SIZE_MAX : previous + bytes;
    }
}

void *scoop_heap_reserve_old_slot(ScoopGcOldSlotPlan *plan, size_t size,
                                 ScoopGcBlockMeta **target);
void scoop_heap_rollback_old_slots(void);
void scoop_heap_finish_old_slot_reservations(void);
void scoop_heap_clear_old_slots(void);
void scoop_heap_record_old_slots(ScoopGcBlockMeta *block);

#endif
