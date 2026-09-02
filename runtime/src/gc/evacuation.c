/* Collector-only evacuation, forwarding and source retirement. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"

static size_t usable_bytes(const ScoopGcBlockMeta *block) {
    return block->kind == SCOOP_BLOCK_KIND_SMALL
               ? GC_BLOCK_SIZE - GC_LINE_SIZE
               : (size_t)block->span_blocks * GC_BLOCK_SIZE - GC_LINE_SIZE;
}

void scoop_gc_heap_plan_moving_locked(void) {
    if (!collection_active) {
        heap_fatal("evacuation planned outside collection");
    }
    uint32_t selected = UINT32_MAX;
    for (uint32_t index = 0; index < arena_next_block; index++) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block) ||
            block->state == SCOOP_BLOCK_EVACUATION_TARGET ||
            block->movable_live_bytes == 0) {
            continue;
        }
        if (selected == UINT32_MAX) {
            selected = index;
            continue;
        }
        ScoopGcBlockMeta *best = &blocks[selected];
        if (block->live_bytes * usable_bytes(best) <
            best->live_bytes * usable_bytes(block)) {
            selected = index;
        }
    }
    if (selected == UINT32_MAX) {
        return;
    }
    ScoopGcBlockMeta *source = &blocks[selected];
    source->state = SCOOP_BLOCK_EVACUATION_SOURCE;
    if (source->kind == SCOOP_BLOCK_KIND_SMALL) {
        source->forwarding =
            calloc(GC_WORDS_PER_BLOCK, sizeof(void *));
        if (source->forwarding == NULL) {
            heap_fatal("out of memory allocating forwarding metadata");
        }
    }
}

static void *evacuate_allocate_small(size_t size, uint32_t *block_index) {
    for (;;) {
        if (evacuation_cursor != NULL) {
            char *line_end =
                (char *)(((uintptr_t)evacuation_cursor &
                          ~(uintptr_t)(GC_LINE_SIZE - 1)) +
                         GC_LINE_SIZE);
            char *candidate = evacuation_cursor + size <= line_end
                                  ? evacuation_cursor
                                  : line_end;
            if (candidate + size <= evacuation_limit) {
                evacuation_cursor = candidate + size;
                *block_index = evacuation_block;
                return candidate;
            }
        }
        evacuation_block =
            activate_small_block(SCOOP_BLOCK_EVACUATION_TARGET);
        if (evacuation_block == UINT32_MAX) {
            heap_fatal("to-space exhausted while evacuating small objects");
        }
        evacuation_cursor =
            (char *)block_base(evacuation_block) + GC_LINE_SIZE;
        evacuation_limit =
            (char *)block_base(evacuation_block) + GC_BLOCK_SIZE;
    }
}

static void *evacuate_allocate(size_t size, uint32_t *block_index) {
    if (size <= GC_SMALL_MAX) {
        return evacuate_allocate_small(size, block_index);
    }
    uint32_t index = activate_large_block(
        size, SCOOP_BLOCK_EVACUATION_TARGET);
    if (index == UINT32_MAX) {
        heap_fatal("to-space exhausted while evacuating a large object");
    }
    *block_index = index;
    return (char *)block_base(index) + GC_LINE_SIZE;
}

void *scoop_gc_forward_object_locked(void *object) {
    if (!collection_active) {
        heap_fatal("object forwarded outside collection");
    }
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("forward requested for a non-object address");
    }
    ScoopGcBlockMeta *source = &blocks[block_index];
    if (!object_marked(source, word_index)) {
        heap_fatal("unmarked object reached relocation");
    }
    if (source->state != SCOOP_BLOCK_EVACUATION_SOURCE ||
        object_pinned(source, word_index)) {
        return object;
    }
    void *forwarded = source->kind == SCOOP_BLOCK_KIND_LARGE
                          ? source->large_forwarding
                          : source->forwarding[word_index];
    if (forwarded != NULL) {
        return forwarded;
    }
    size_t size = scoop_gc_object_size_locked(object);
    uint32_t target_index;
    forwarded = evacuate_allocate(size, &target_index);
    memcpy(forwarded, object, size);
    if (size <= GC_SMALL_MAX) {
        record_small_object(target_index, forwarded, size, true);
    } else {
        publish_large_object(target_index, true);
    }
    if (source->kind == SCOOP_BLOCK_KIND_LARGE) {
        source->large_forwarding = forwarded;
    } else {
        source->forwarding[word_index] = forwarded;
    }
    moved_objects++;
    return forwarded;
}

bool scoop_gc_claim_object_scan_locked(void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("scan claimed for a non-object address");
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    bool current = object_marked(block, word_index) &&
                   !(block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                     !object_pinned(block, word_index));
    if (!current) {
        heap_fatal("scan claimed for a non-current object");
    }
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (block->large_scanned) {
            return false;
        }
        block->large_scanned = true;
        return true;
    }
    if (bit_test(block->scanned, word_index)) {
        return false;
    }
    bit_set(block->scanned, word_index);
    return true;
}

bool scoop_gc_object_was_scanned_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    return block->kind == SCOOP_BLOCK_KIND_LARGE
               ? block->large_scanned
               : bit_test(block->scanned, word_index);
}

bool scoop_gc_is_forwarded_old_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE ||
        object_pinned(block, word_index)) {
        return false;
    }
    return block->kind == SCOOP_BLOCK_KIND_LARGE
               ? block->large_forwarding != NULL
               : block->forwarding[word_index] != NULL;
}

bool scoop_gc_is_current_live_object_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    return object_marked(block, word_index) &&
           !(block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
             !object_pinned(block, word_index));
}

void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor,
                                            void *context) {
    if (visitor == NULL || !collection_active) {
        heap_fatal("invalid live-object visitor");
    }
    for (uint32_t index = 0; index < arena_next_block; index++) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block)) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked) {
                if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                    !block->large_pinned) {
                    if (block->large_forwarding == NULL) {
                        heap_fatal("live large source object was not forwarded");
                    }
                } else {
                    visitor((char *)block_base(index) + GC_LINE_SIZE,
                            context);
                }
            }
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t);
             word < GC_WORDS_PER_BLOCK; word++) {
            if (!bit_test(block->starts, word) ||
                !bit_test(block->marks, word)) {
                continue;
            }
            if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                !bit_test(block->pins, word)) {
                if (block->forwarding[word] == NULL) {
                    heap_fatal("live small source object was not forwarded");
                }
                continue;
            }
            visitor((char *)block_base(index) +
                        word * sizeof(uint64_t),
                    context);
        }
    }
}

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

static bool finish_small_block(uint32_t index) {
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
        release_block(index);
        return false;
    }
    block->state = any_pinned ? SCOOP_BLOCK_PINNED_PARTIAL
                              : SCOOP_BLOCK_MUTATOR;
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

static bool finish_large_block(uint32_t index) {
    ScoopGcBlockMeta *block = &blocks[index];
    bool moved = block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                 !block->large_pinned;
    if (moved && block->large_marked && block->large_forwarding == NULL) {
        heap_fatal("marked large source object has no forwarding address");
    }
    bool keep = block->large_marked && !moved;
    if (!keep) {
        release_block(index);
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
            (void)finish_small_block(index);
        } else if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            (void)finish_large_block(index);
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
