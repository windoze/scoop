/* Post-verification object retirement, ordinary reuse and stress quarantine. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "../platform/platform.h"

static void free_run_push(uint32_t block_index, size_t first_line,
                          size_t line_count) {
    if (line_count == 0 || first_line == 0 ||
        first_line + line_count > GC_LINES_PER_BLOCK) {
        heap_fatal("invalid free line run");
    }
    ScoopGcFreeRun *run = malloc(sizeof *run);
    if (run == NULL) {
        heap_fatal("out of memory recording a free line run");
    }
    *run = (ScoopGcFreeRun){
        .next = free_runs,
        .block_index = block_index,
        .first_line = (uint16_t)first_line,
        .line_count = (uint16_t)line_count,
    };
    free_runs = run;
}

static void release_block(uint32_t index) {
    ScoopGcBlockMeta *head = &blocks[index];
    uint32_t span = head->span_blocks;
    if (!active_head(head) || span == 0 ||
        index > GC_BLOCK_COUNT - span) {
        heap_fatal("invalid active block release");
    }
    free(head->forwarding);
    head->forwarding = NULL;
    for (uint32_t offset = 0; offset < span; offset++) {
        ScoopGcBlockMeta *block = &blocks[index + offset];
        block->state = SCOOP_BLOCK_FREE;
        block->kind = SCOOP_BLOCK_KIND_NONE;
        block->span_blocks = 0;
        block->owner_block = 0;
        block->exact_size = 0;
        block->live_bytes = 0;
        block->movable_live_bytes = 0;
        block->large_forwarding = NULL;
        block->large_published = false;
        block->large_marked = false;
        block->large_pinned = false;
        block->large_scanned = false;
    }
    committed_bytes -= (size_t)span * GC_BLOCK_SIZE;
    active_block_heads--;
    free_span_insert(index, span);
}

static void protect_quarantined_span(uint32_t index, uint32_t span) {
    const ScoopPlatformBundle *bundle = scoop_platform_bundle();
    size_t page_size = bundle->thread_vm->page_size();
    void *base = block_base(index);
    size_t size = (size_t)span * GC_BLOCK_SIZE;
    if (page_size == 0 || GC_BLOCK_SIZE % page_size != 0 ||
        (uintptr_t)base % GC_BLOCK_SIZE != 0 ||
        (uintptr_t)base % page_size != 0 || size % page_size != 0) {
        heap_fatal(
            "GC block span is not compatible with platform page protection");
    }
    ScoopPlatformError error = {0};
    if (!bundle->thread_vm->protect_none(base, size, &error)) {
        heap_fatal(scoop_platform_error_message(error.code));
    }
}

static void quarantine_block(uint32_t index) {
    ScoopGcBlockMeta *head = &blocks[index];
    uint32_t span = head->span_blocks;
    ScoopGcBlockKind kind = head->kind;
    if (!active_head(head) || span == 0 ||
        index > GC_BLOCK_COUNT - span) {
        heap_fatal("invalid active block quarantine");
    }
    protect_quarantined_span(index, span);
    free(head->forwarding);
    head->forwarding = NULL;
    if (kind == SCOOP_BLOCK_KIND_SMALL) {
        memset(head->starts, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
        memset(head->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
        memset(head->pins, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
        memset(head->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
        memset(head->line_occupied, 0,
               GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
        memset(head->line_live, 0,
               GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
        memset(head->size_units, 0,
               GC_WORDS_PER_BLOCK * sizeof(uint8_t));
    }
    for (uint32_t offset = 0; offset < span; offset++) {
        ScoopGcBlockMeta *block = &blocks[index + offset];
        block->state = SCOOP_BLOCK_QUARANTINED;
        block->kind = SCOOP_BLOCK_KIND_NONE;
        block->span_blocks = 0;
        block->owner_block = index;
        block->exact_size = 0;
        block->live_bytes = 0;
        block->movable_live_bytes = 0;
        block->large_forwarding = NULL;
        block->large_published = false;
        block->large_marked = false;
        block->large_pinned = false;
        block->large_scanned = false;
    }
    committed_bytes -= (size_t)span * GC_BLOCK_SIZE;
    active_block_heads--;
}

static bool finish_small_block(uint32_t index, bool stress) {
    ScoopGcBlockMeta *block = &blocks[index];
    memset(block->line_occupied, 0,
           GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
    bool any_live = false;
    bool any_pinned = false;
    for (size_t word = GC_LINE_SIZE / sizeof(uint64_t);
         word < GC_WORDS_PER_BLOCK; word++) {
        if (!bit_test(block->starts, word)) {
            continue;
        }
        bool pinned = bit_test(block->pins, word);
        bool marked = bit_test(block->marks, word);
        bool moved = block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                     !pinned;
        if (moved && marked && block->forwarding[word] == NULL) {
            heap_fatal("marked source object has no forwarding address");
        }
        bool keep = marked && !moved;
        if (!keep) {
            size_t size =
                (size_t)block->size_units[word] * sizeof(uint64_t);
            if (size == 0) {
                heap_fatal("allocated object lost its exact size");
            }
            if (stress) {
                memset((char *)block_base(index) +
                           word * sizeof(uint64_t),
                       GC_POISON_BYTE, size);
            }
            bit_clear(block->starts, word);
            bit_clear(block->pins, word);
            block->size_units[word] = 0;
            continue;
        }
        size_t size = (size_t)block->size_units[word] * sizeof(uint64_t);
        if (size == 0) {
            heap_fatal("live object lost its exact size");
        }
        uintptr_t first = word * sizeof(uint64_t);
        uintptr_t last = first + size - 1;
        for (size_t line = first / GC_LINE_SIZE;
             line <= last / GC_LINE_SIZE; line++) {
            bit_set(block->line_occupied, line);
        }
        any_live = true;
        any_pinned |= pinned;
    }
    memset(block->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->line_live, 0,
           GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
    free(block->forwarding);
    block->forwarding = NULL;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    if (!any_live) {
        if (stress) {
            quarantine_block(index);
        } else {
            release_block(index);
        }
        return false;
    }
    block->state = any_pinned ? SCOOP_BLOCK_PINNED_PARTIAL
                              : SCOOP_BLOCK_MUTATOR;
    if (stress) {
        return true;
    }
    size_t run_start = 0;
    for (size_t line = 1; line <= GC_LINES_PER_BLOCK; line++) {
        bool occupied = line < GC_LINES_PER_BLOCK
                            ? bit_test(block->line_occupied, line)
                            : true;
        if (!occupied && run_start == 0) {
            run_start = line;
        } else if (occupied && run_start != 0) {
            free_run_push(index, run_start, line - run_start);
            run_start = 0;
        }
    }
    return true;
}

static bool finish_large_block(uint32_t index, bool stress) {
    ScoopGcBlockMeta *block = &blocks[index];
    bool moved = block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                 !block->large_pinned;
    if (moved && block->large_marked && block->large_forwarding == NULL) {
        heap_fatal("marked large source object has no forwarding address");
    }
    bool keep = block->large_marked && !moved;
    if (!keep) {
        if (stress) {
            memset((char *)block_base(index) + GC_LINE_SIZE,
                   GC_POISON_BYTE, block->exact_size);
            quarantine_block(index);
        } else {
            release_block(index);
        }
        return false;
    }
    block->state = block->large_pinned ? SCOOP_BLOCK_PINNED_PARTIAL
                                       : SCOOP_BLOCK_MUTATOR;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    block->large_forwarding = NULL;
    block->large_marked = false;
    block->large_scanned = false;
    return true;
}

void scoop_gc_heap_finish_collection_locked(uint64_t object_count) {
    if (!collection_active) {
        heap_fatal("heap collection finished without begin");
    }
    free_run_nodes();
    for (uint32_t index = 0; index < arena_next_block;) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block)) {
            index++;
            continue;
        }
        uint32_t span = block->span_blocks;
        if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
            (void)finish_small_block(index, stress_move);
        } else if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            (void)finish_large_block(index, stress_move);
        } else {
            heap_fatal("active block has no object kind");
        }
        index += span;
    }
    memset(card_table_storage, 0, GC_CARD_TABLE_SIZE);
    atomic_store_explicit(&live_objects, object_count,
                          memory_order_release);
    atomic_store_explicit(&last_moved_objects, moved_objects,
                          memory_order_release);
    collection_threshold = committed_bytes * 2 > GC_INITIAL_THRESHOLD
                               ? committed_bytes * 2
                               : GC_INITIAL_THRESHOLD;
    evacuation_block = UINT32_MAX;
    evacuation_cursor = NULL;
    evacuation_limit = NULL;
    collection_active = false;
}
