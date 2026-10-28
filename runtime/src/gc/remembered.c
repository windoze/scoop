/* Enumerate only old objects intersecting dirty cards, including prior starts. */
#include <string.h>

#include "../gc_binding/object.h"
#include "gc_internal.h"
#include "heap_internal.h"

static size_t previous_start(const ScoopGcBlockMeta *block, size_t word) {
    size_t bitmap_word = word / 64;
    uint64_t mask = UINT64_MAX >> (63 - word % 64);
    for (;;) {
        uint64_t bits = block->starts[bitmap_word] & mask;
        if (bits != 0) {
            return bitmap_word * 64 + 63 - (size_t)__builtin_clzll(bits);
        }
        if (bitmap_word == 0) {
            return GC_LINE_SIZE / sizeof(uint64_t);
        }
        bitmap_word--;
        mask = UINT64_MAX;
    }
}

uint64_t scoop_gc_scan_dirty_group(ScoopGcCardGroup *group, ScoopGcObjectRangeVisitor visitor,
                                   void *context, bool only_old) {
    ScoopGcRegion *region = group->region;
    size_t index = (size_t)(group - region->card_groups);
    ScoopGcBlockMeta *block = &region->blocks[region->large ? 0 : index];
    if (!active_head(block) || (only_old && block->generation != SCOOP_GC_OLD)) {
        return 0;
    }
    uintptr_t base = (uintptr_t)block_base(block);
    if (region->large) {
        if (!block->large_published ||
            scoop_gc_object_scan((const void *)(base + GC_LINE_SIZE)) == NULL) {
            return 0;
        }
    }
    size_t first_card = index * GC_CARDS_PER_GROUP;
    size_t count = first_card + GC_CARDS_PER_GROUP;
    uint64_t visited = 0;
    for (size_t card = first_card; card < count;) {
        if (region->cards[card] == 0) {
            const unsigned char *dirty = memchr(region->cards + card, 1, count - card);
            if (dirty == NULL) {
                break;
            }
            card = (size_t)(dirty - region->cards);
        }
        uintptr_t begin = region->base + (card << GC_CARD_SHIFT);
        const unsigned char *clean = memchr(region->cards + card, 0, count - card);
        size_t last_card = clean == NULL ? count : (size_t)(clean - region->cards);
        uintptr_t end = region->base + (last_card << GC_CARD_SHIFT);
        visited += last_card - card;
        card = last_card;
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            visitor((void *)(base + GC_LINE_SIZE), begin, end, context);
            continue;
        }
        size_t first = previous_start(block, (begin - base) / sizeof(uint64_t));
        size_t last = (end - base) / sizeof(uint64_t);
        for (size_t index = first / 64; index <= (last - 1) / 64; index++) {
            uint64_t bits = block->starts[index];
            if (index == first / 64) {
                bits &= UINT64_MAX << (first % 64);
            }
            if (index == (last - 1) / 64) {
                bits &= UINT64_MAX >> (63 - (last - 1) % 64);
            }
            while (bits != 0) {
                size_t word = index * 64 + (size_t)__builtin_ctzll(bits);
                bits &= bits - 1;
                uintptr_t object = base + word * sizeof(uint64_t);
                size_t size = scoop_heap_small_object_size((const void *)object);
                if (object + size > begin) {
                    visitor((void *)object, begin, end, context);
                }
            }
        }
    }
    return visited;
}

void scoop_gc_scan_remembered(ScoopGcObjectRangeVisitor visitor, void *context, bool count_cards) {
    ScoopGcCardGroup *first =
        atomic_load_explicit(&scoop_gc_heap_state.dirty_card_groups, memory_order_acquire);
    for (ScoopGcCardGroup *group = first; group != NULL;
         group = scoop_heap_next_dirty_card_group(group)) {
        uint64_t visited = scoop_gc_scan_dirty_group(group, visitor, context, true);
        if (count_cards) {
            scoop_gc_heap_state.metrics.dirty_cards += visited;
        }
    }
}
