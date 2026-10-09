/* Dead-object release, live line reconstruction and reusable evacuation holes. */
#include <stdlib.h>
#include <string.h>

#include "heap_internal.h"

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

static void record_free_runs(ScoopGcBlockMeta *block, const uint64_t *occupied_lines) {
    size_t run_start = 0;
    for (size_t line = 1; line <= GC_LINES_PER_BLOCK; line++) {
        bool occupied = line < GC_LINES_PER_BLOCK ? bit_test(occupied_lines, line) : true;
        if (!occupied && run_start == 0) {
            run_start = line;
        } else if (occupied && run_start != 0) {
            free_run_push(block, run_start, line - run_start);
            run_start = 0;
        }
    }
}

/* Every dead nursery object reaches this helper; avoid an extra per-object call. */
static inline __attribute__((always_inline)) void
retire_small_object(ScoopGcBlockMeta *block, size_t word, bool marked, bool stress) {
    size_t size = (size_t)block->size_units[word] * sizeof(uint64_t);
    if (size == 0) {
        heap_fatal("allocated object lost its exact size");
    }
    void *object = (char *)block_base(block) + word * sizeof(uint64_t);
    if (!marked) {
        release_dead_object(object);
    }
    if (stress) {
        memset(object, GC_POISON_BYTE, size);
    }
    bit_clear(block->starts, word);
    bit_clear(block->pins, word);
    block->size_units[word] = 0;
    block->discard_pending = true;
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
            retire_small_object(block, word, marked, stress);
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
    record_free_runs(block, block->line_occupied);
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

void scoop_heap_prepare_evacuation_targets(void) {
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || block->kind != SCOOP_BLOCK_KIND_SMALL ||
            block->region->evacuation_source ||
            (block->generation == SCOOP_GC_YOUNG && block->live_bytes != 0)) {
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK; word++) {
            if (bit_test(block->starts, word) && !bit_test(block->marks, word)) {
                retire_small_object(block, word, false, false);
            }
        }
        if (block->live_bytes == 0) {
            scoop_heap_release_block(block);
        } else {
            record_free_runs(block, block->line_live);
        }
    }
}

void scoop_heap_finish_block(ScoopGcBlockMeta *block, bool stress) {
    if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
        (void)finish_small_block(block, stress);
    } else if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        (void)finish_large_block(block, stress);
    } else {
        heap_fatal("active block has no object kind");
    }
}
