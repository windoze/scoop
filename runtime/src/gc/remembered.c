/* Enumerate only old objects intersecting dirty cards, including prior starts. */
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

void scoop_gc_scan_remembered(ScoopGcObjectRangeVisitor visitor, void *context, bool count_cards) {
    size_t card_count = (size_t)arena_next_block * (GC_BLOCK_SIZE >> GC_CARD_SHIFT);
    for (size_t card = 0; card < card_count; card++) {
        if (card_table_storage[card] == 0) {
            continue;
        }
        uintptr_t begin = arena_base + (card << GC_CARD_SHIFT);
        uintptr_t end = begin + ((uintptr_t)1 << GC_CARD_SHIFT);
        uint32_t index = (uint32_t)((begin - arena_base) / GC_BLOCK_SIZE);
        ScoopGcBlockMeta *block = &blocks[index];
        if (block->state == SCOOP_BLOCK_LARGE_TAIL) {
            index = block->owner_block;
            block = &blocks[index];
        }
        if (!active_head(block) || block->generation != SCOOP_GC_OLD) {
            continue;
        }
        if (count_cards) {
            scoop_gc_heap_state.metrics.dirty_cards++;
        }
        uintptr_t base = (uintptr_t)block_base(index);
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            visitor((void *)(base + GC_LINE_SIZE), begin, end, context);
            continue;
        }
        size_t first = previous_start(block, (begin - base) / sizeof(uint64_t));
        size_t last = (end - base) / sizeof(uint64_t);
        for (size_t word = first; word < last; word++) {
            if (!bit_test(block->starts, word)) {
                continue;
            }
            uintptr_t object = base + word * sizeof(uint64_t);
            size_t size = (size_t)block->size_units[word] * sizeof(uint64_t);
            if (object + size > begin) {
                visitor((void *)object, begin, end, context);
            }
        }
    }
}
