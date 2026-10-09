/* Post-verification object retirement, ordinary reuse and stress quarantine. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "../platform/platform.h"
#include "../thread.h"

static void release_dead_object(void *object) {
    ScoopObjectHeader *header = object;
    ScoopReleaseHookV1 hook = header->td->release_hook;
    if (hook == NULL) {
        if (__atomic_load_n(&header->gc_word, __ATOMIC_ACQUIRE) & GC_RELEASE_READY_BIT) {
            heap_fatal("release-ready object has no release hook");
        }
        return;
    }
    uint64_t previous =
        __atomic_fetch_and(&header->gc_word, ~GC_RELEASE_READY_BIT, __ATOMIC_ACQ_REL);
    if (previous & GC_RELEASE_READY_BIT) {
        hook(object);
    }
}

static void free_run_push(ScoopGcBlockMeta *block, size_t first_line, size_t line_count) {
    if (line_count == 0 || first_line == 0 || first_line + line_count > GC_LINES_PER_BLOCK) {
        heap_fatal("invalid free line run");
    }
    ScoopGcFreeRun *run = malloc(sizeof *run);
    if (run == NULL) {
        heap_fatal("out of memory recording a free line run");
    }
    *run = (ScoopGcFreeRun){
        .next = free_runs,
        .block = block,
        .first_line = (uint16_t)first_line,
        .line_count = (uint16_t)line_count,
    };
    free_runs = run;
}

static bool finish_small_block(ScoopGcBlockMeta *block, bool stress) {
    memset(block->line_occupied, 0, GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
    bool any_live = false;
    bool any_pinned = false;
    for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK; word++) {
        if (!bit_test(block->starts, word)) {
            continue;
        }
        bool pinned = bit_test(block->pins, word);
        bool marked = bit_test(block->marks, word);
        bool moved = block->state == SCOOP_BLOCK_EVACUATION_SOURCE && !pinned;
        if (moved && marked && block->forwarding[word] == NULL) {
            heap_fatal("marked source object has no forwarding address");
        }
        bool keep = marked && !moved;
        if (!keep) {
            size_t size = (size_t)block->size_units[word] * sizeof(uint64_t);
            if (size == 0) {
                heap_fatal("allocated object lost its exact size");
            }
            if (!marked) {
                release_dead_object((char *)block_base(block) + word * sizeof(uint64_t));
            }
            if (stress) {
                memset((char *)block_base(block) + word * sizeof(uint64_t), GC_POISON_BYTE, size);
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
        for (size_t line = first / GC_LINE_SIZE; line <= last / GC_LINE_SIZE; line++) {
            bit_set(block->line_occupied, line);
        }
        any_live = true;
        any_pinned |= pinned;
    }
    memset(block->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->line_live, 0, GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
    free(block->forwarding);
    block->forwarding = NULL;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    if (!any_live) {
        if (stress) {
            scoop_heap_quarantine_block(block);
        } else {
            scoop_heap_release_block(block);
        }
        return false;
    }
    block->state = any_pinned ? SCOOP_BLOCK_PINNED_PARTIAL : SCOOP_BLOCK_MUTATOR;
    if (stress) {
        return true;
    }
    size_t run_start = 0;
    for (size_t line = 1; line <= GC_LINES_PER_BLOCK; line++) {
        bool occupied = line < GC_LINES_PER_BLOCK ? bit_test(block->line_occupied, line) : true;
        if (!occupied && run_start == 0) {
            run_start = line;
        } else if (occupied && run_start != 0) {
            free_run_push(block, run_start, line - run_start);
            run_start = 0;
        }
    }
    return true;
}

static bool finish_large_block(ScoopGcBlockMeta *block, bool stress) {
    bool moved = block->state == SCOOP_BLOCK_EVACUATION_SOURCE && !block->large_pinned;
    if (moved && block->large_marked && block->large_forwarding == NULL) {
        heap_fatal("marked large source object has no forwarding address");
    }
    bool keep = block->large_marked && !moved;
    if (!keep) {
        if (!block->large_marked) {
            release_dead_object((char *)block_base(block) + GC_LINE_SIZE);
        }
        if (stress) {
            memset((char *)block_base(block) + GC_LINE_SIZE, GC_POISON_BYTE, block->exact_size);
            scoop_heap_quarantine_block(block);
        } else {
            scoop_heap_release_block(block);
        }
        return false;
    }
    block->state = block->large_pinned ? SCOOP_BLOCK_PINNED_PARTIAL : SCOOP_BLOCK_MUTATOR;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    block->large_forwarding = NULL;
    block->large_marked = false;
    block->large_scanned = false;
    return true;
}

void scoop_gc_heap_finish_collection_locked(uint64_t object_count, bool minor) {
    if (!collection_active) {
        heap_fatal("heap collection finished without begin");
    }
    ScoopAllocationTotals allocations = scoop_thread_allocation_totals_locked();
    if (minor) {
        uint64_t allocated =
            allocations.objects - scoop_gc_heap_state.allocation_objects_at_collection;
        uint64_t young =
            allocations.nursery_objects - scoop_gc_heap_state.nursery_objects_at_collection;
        object_count += scoop_gc_heap_state.collected_live_objects + allocated - young;
    } else {
        free_run_nodes();
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || (minor && block->generation == SCOOP_GC_OLD &&
                                    block->state != SCOOP_BLOCK_EVACUATION_TARGET)) {
            continue;
        }
        if (block->generation == SCOOP_GC_YOUNG) {
            scoop_gc_heap_state.metrics.promoted_bytes += block->live_bytes;
        }
        block->generation = SCOOP_GC_OLD;
        if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
            (void)finish_small_block(block, stress_move);
        } else if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            (void)finish_large_block(block, stress_move);
        } else {
            heap_fatal("active block has no object kind");
        }
    }
    evacuation_block = NULL;
    evacuation_cursor = NULL;
    evacuation_limit = NULL;
    scoop_heap_release_empty_large_regions();
    for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        memset(region->cards, 0, region->size >> GC_CARD_SHIFT);
    }
    scoop_gc_heap_state.nursery_bytes = 0;
    scoop_gc_heap_state.collected_live_objects = object_count;
    scoop_gc_heap_state.allocation_objects_at_collection = allocations.objects;
    scoop_gc_heap_state.nursery_objects_at_collection = allocations.nursery_objects;
    atomic_store_explicit(&last_moved_objects, moved_objects, memory_order_release);
    if (!minor) {
        size_t next = committed_bytes > SIZE_MAX / 2 ? SIZE_MAX : committed_bytes * 2;
        collection_threshold = next > GC_INITIAL_THRESHOLD ? next : GC_INITIAL_THRESHOLD;
    }
    collection_active = false;
}
