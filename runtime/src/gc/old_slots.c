/* Collector-only old slots. Dead object bytes never hold persistent free links. */
#include <stdlib.h>
#include <string.h>

#include "old_slots.h"

struct ScoopGcOldSlots {
    ScoopGcOldSlots *next;
    ScoopGcBlockMeta *block;
    uint16_t cursor;
    uint16_t remaining;
    bool fresh;
};

static size_t class_index(size_t size) { return size / sizeof(uint64_t) - 2; }

bool scoop_heap_select_slot_source(ScoopGcBlockMeta *block, size_t *available, bool selected) {
    size_t size = block->nursery_slot_size;
    if (size == 0 || size > GC_OLD_SLOT_MAX) {
        return selected;
    }
    size_t *free_slots = &available[class_index(size)];
    size_t live_slots = block->live_bytes / size;
    if (*free_slots >= live_slots) {
        *free_slots -= live_slots;
        return true;
    }
    block->slot_size = size;
    return false;
}

static uint16_t empty_slots(const ScoopGcBlockMeta *block) {
    size_t count = GC_REGULAR_MAX / block->slot_size;
    for (size_t index = 0; index < GC_BITMAP_WORDS; index++) {
        count -= (size_t)__builtin_popcountll(block->starts[index]);
    }
    return (uint16_t)count;
}

static bool seek_empty(ScoopGcOldSlots *slots) {
    size_t step = slots->block->slot_size / sizeof(uint64_t);
    while ((size_t)slots->cursor + step <= GC_WORDS_PER_BLOCK) {
        if (!bit_test(slots->block->starts, slots->cursor)) {
            return true;
        }
        slots->cursor += step;
    }
    return false;
}

static void make_available(ScoopGcOldSlots *slots) {
    if (!seek_empty(slots)) {
        free(slots);
        return;
    }
    size_t index = class_index(slots->block->slot_size);
    slots->fresh = false;
    scoop_gc_heap_state.old_slots_free[index] += slots->remaining;
    slots->next = scoop_gc_heap_state.old_slots_available[index];
    scoop_gc_heap_state.old_slots_available[index] = slots;
}

void scoop_heap_record_old_slots(ScoopGcBlockMeta *block) {
    ScoopGcOldSlots probe = {
        .block = block,
        .cursor = GC_LINE_SIZE / sizeof(uint64_t),
        .remaining = empty_slots(block),
    };
    if (!seek_empty(&probe)) {
        return;
    }
    ScoopGcOldSlots *slots = malloc(sizeof *slots);
    if (slots == NULL) {
        heap_fatal("out of memory recording an old slot block");
    }
    *slots = probe;
    make_available(slots);
}

void *scoop_heap_reserve_old_slot(ScoopGcOldSlotPlan *plan, size_t size,
                                 ScoopGcBlockMeta **target, pthread_mutex_t *reservation_lock) {
    size_t index = class_index(size);
    for (;;) {
        ScoopGcOldSlots *slots = plan->current[index];
        if (slots == NULL) {
            if (reservation_lock != NULL) {
                pthread_mutex_lock(reservation_lock);
            }
            slots = scoop_gc_heap_state.old_slots_available[index];
            if (slots != NULL) {
                scoop_gc_heap_state.old_slots_available[index] = slots->next;
                scoop_gc_heap_state.old_slots_free[index] -= slots->remaining;
            } else {
                ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_EVACUATION_TARGET, true);
                if (block == NULL) {
                    if (reservation_lock != NULL) {
                        pthread_mutex_unlock(reservation_lock);
                    }
                    return NULL;
                }
                block->slot_size = size;
                slots = malloc(sizeof *slots);
                if (slots == NULL) {
                    heap_fatal("out of memory reserving an old slot block");
                }
                *slots = (ScoopGcOldSlots){
                    .block = block,
                    .cursor = GC_LINE_SIZE / sizeof(uint64_t),
                    .remaining = (uint16_t)(GC_REGULAR_MAX / size),
                    .fresh = true,
                };
            }
            slots->next = scoop_gc_heap_state.old_slots_reserved;
            scoop_gc_heap_state.old_slots_reserved = slots;
            plan->current[index] = slots;
            if (reservation_lock != NULL) {
                pthread_mutex_unlock(reservation_lock);
            }
        }
        if (seek_empty(slots)) {
            *target = slots->block;
            void *object = (char *)block_base(*target) + (size_t)slots->cursor * sizeof(uint64_t);
            slots->cursor += size / sizeof(uint64_t);
            slots->remaining--;
            return object;
        }
        plan->current[index] = NULL;
    }
}

void scoop_heap_rollback_old_slots(void) {
    while (scoop_gc_heap_state.old_slots_reserved != NULL) {
        ScoopGcOldSlots *slots = scoop_gc_heap_state.old_slots_reserved;
        scoop_gc_heap_state.old_slots_reserved = slots->next;
        if (slots->fresh) {
            /* The normal reservation rollback releases this unpublished block. */
            free(slots);
        } else {
            slots->cursor = GC_LINE_SIZE / sizeof(uint64_t);
            slots->remaining = empty_slots(slots->block);
            make_available(slots);
        }
    }
}

void scoop_heap_finish_old_slot_reservations(void) {
    while (scoop_gc_heap_state.old_slots_reserved != NULL) {
        ScoopGcOldSlots *slots = scoop_gc_heap_state.old_slots_reserved;
        scoop_gc_heap_state.old_slots_reserved = slots->next;
        if (!slots->fresh) {
            ScoopGcBlockMeta *block = slots->block;
            /* Existing old objects did not participate in the young mark. */
            memset(block->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
            memset(block->line_live, 0, GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
#if SCOOP_GC_TRACK_RELOCATION
            memset(block->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
#endif
            block->live_bytes = 0;
            block->movable_live_bytes = 0;
            block->discard_pending = true;
        }
        make_available(slots);
    }
}

void scoop_heap_clear_old_slots(void) {
    for (size_t index = 0; index < GC_OLD_SLOT_CLASSES; index++) {
        ScoopGcOldSlots *slots = scoop_gc_heap_state.old_slots_available[index];
        scoop_gc_heap_state.old_slots_available[index] = NULL;
        scoop_gc_heap_state.old_slots_free[index] = 0;
        while (slots != NULL) {
            ScoopGcOldSlots *next = slots->next;
            free(slots);
            slots = next;
        }
    }
}
