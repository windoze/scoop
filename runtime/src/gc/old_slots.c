/* Collector-only old slots. Dead object bytes never hold persistent free links. */
#include <stdlib.h>
#include <string.h>

#include "old_slots.h"

struct ScoopGcOldSlots {
    ScoopGcOldSlots *next;
    ScoopGcBlockMeta *block;
    uint64_t free_starts;
    uint16_t cursor;
    uint16_t remaining;
    uint16_t bitmap_word;
    bool fresh;
};

/* Relative starts for the exact 2..32-word slot classes. */
static const uint64_t slot_word_patterns[] = {
    UINT64_C(0x5555555555555555), UINT64_C(0x9249249249249249), UINT64_C(0x1111111111111111),
    UINT64_C(0x1084210842108421), UINT64_C(0x1041041041041041), UINT64_C(0x8102040810204081),
    UINT64_C(0x0101010101010101), UINT64_C(0x8040201008040201), UINT64_C(0x1004010040100401),
    UINT64_C(0x0080100200400801), UINT64_C(0x1001001001001001), UINT64_C(0x0010008004002001),
    UINT64_C(0x0100040010004001), UINT64_C(0x1000200040008001), UINT64_C(0x0001000100010001),
    UINT64_C(0x0008000400020001), UINT64_C(0x0040001000040001), UINT64_C(0x0200004000080001),
    UINT64_C(0x1000010000100001), UINT64_C(0x8000040000200001), UINT64_C(0x0000100000400001),
    UINT64_C(0x0000400000800001), UINT64_C(0x0001000001000001), UINT64_C(0x0004000002000001),
    UINT64_C(0x0010000004000001), UINT64_C(0x0040000008000001), UINT64_C(0x0100000010000001),
    UINT64_C(0x0400000020000001), UINT64_C(0x1000000040000001), UINT64_C(0x4000000080000001),
    UINT64_C(0x0000000100000001),
};
_Static_assert(sizeof slot_word_patterns / sizeof *slot_word_patterns == GC_OLD_SLOT_CLASSES,
               "each exact slot class has a bitmap pattern");

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
    if (slots->free_starts != 0) {
        return true;
    }
    if (slots->remaining == 0) {
        return false;
    }
    size_t step = slots->block->slot_size / sizeof(uint64_t);
    uint64_t pattern = slot_word_patterns[step - 2];
    while ((size_t)slots->cursor + step <= GC_WORDS_PER_BLOCK) {
        size_t bit = slots->cursor % 64;
        size_t bitmap_word = slots->cursor / 64;
        uint64_t candidates = pattern << bit;
        if (bitmap_word == GC_BITMAP_WORDS - 1) {
            candidates &= UINT64_MAX >> (step - 1);
        }
        slots->free_starts = candidates & ~slots->block->starts[bitmap_word];
        slots->bitmap_word = (uint16_t)bitmap_word;
        slots->cursor += ((63 - bit) / step + 1) * step;
        if (slots->free_starts != 0) {
            return true;
        }
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
            size_t word = (size_t)slots->bitmap_word * 64 +
                          (size_t)__builtin_ctzll(slots->free_starts);
            slots->free_starts &= slots->free_starts - 1;
            slots->remaining--;
            return (char *)block_base(*target) + word * sizeof(uint64_t);
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
            slots->free_starts = 0;
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
