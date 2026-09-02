/* Moving Immix heap storage (runtime spec sections 3.1 and 3.7).
 *
 * Every collector datum lives outside the GC arena. Heap addresses identify a
 * block index; the side table owns block state, exact object sizes, start,
 * mark, pin, scan and forwarding metadata. Mutator TLABs contain only object
 * storage, and free spans/runs use malloc-backed nodes. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "../platform/platform.h"

unsigned char *scoop_gc_card_table;
#undef collection_threshold
#undef evacuation_block
ScoopGcHeapState scoop_gc_heap_state = {
    .lock = PTHREAD_MUTEX_INITIALIZER,
    .collection_threshold = GC_INITIAL_THRESHOLD,
    .evacuation_block = UINT32_MAX,
};
#define collection_threshold (scoop_gc_heap_state.collection_threshold)
#define evacuation_block (scoop_gc_heap_state.evacuation_block)

_Noreturn void heap_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

void lock_heap(void) {
    if (pthread_mutex_lock(&heap_lock) != 0) {
        heap_fatal("failed to lock the heap");
    }
}

void unlock_heap(void) {
    if (pthread_mutex_unlock(&heap_lock) != 0) {
        heap_fatal("failed to unlock the heap");
    }
}

void scoop_gc_heap_lock(void) {
    lock_heap();
}

void scoop_gc_heap_unlock(void) {
    unlock_heap();
}

void *block_base(uint32_t index) {
    return (void *)(arena_base + (uintptr_t)index * GC_BLOCK_SIZE);
}

bool active_head(const ScoopGcBlockMeta *block) {
    return block->state == SCOOP_BLOCK_MUTATOR ||
           block->state == SCOOP_BLOCK_EVACUATION_SOURCE ||
           block->state == SCOOP_BLOCK_EVACUATION_TARGET ||
           block->state == SCOOP_BLOCK_PINNED_PARTIAL;
}

void free_run_nodes(void) {
    while (free_runs != NULL) {
        ScoopGcFreeRun *next = free_runs->next;
        free(free_runs);
        free_runs = next;
    }
}

static void arena_init(void) {
    size_t reserve = GC_ARENA_SIZE + GC_BLOCK_SIZE;
    void *mapping = NULL;
    ScoopPlatformError error = {0};
    if (!scoop_platform_bundle()->thread_vm->reserve_read_write(
            GC_ARENA_HINT, reserve, &mapping, &error)) {
        heap_fatal(scoop_platform_error_message(error.code));
    }
    arena_base = ((uintptr_t)mapping + GC_BLOCK_SIZE - 1) &
                 ~(uintptr_t)(GC_BLOCK_SIZE - 1);
    arena_end = (char *)arena_base + GC_ARENA_SIZE;
    blocks = calloc(GC_BLOCK_COUNT, sizeof *blocks);
    card_table_storage = calloc(1, GC_CARD_TABLE_SIZE);
    if (blocks == NULL || card_table_storage == NULL) {
        heap_fatal("out of memory allocating heap side metadata");
    }
    scoop_gc_card_table =
        (unsigned char *)((uintptr_t)card_table_storage -
                          (arena_base >> GC_CARD_SHIFT));
    arena_ready = true;
}

void scoop_gc_heap_init(void) {
    if (arena_ready) {
        heap_fatal("heap initialized more than once");
    }
    const char *stress = getenv("SCOOP_GC_STRESS_MOVE");
    stress_move = stress != NULL && strcmp(stress, "1") == 0;
    arena_init();
}

static void require_arena(void) {
    if (!arena_ready) {
        heap_fatal("heap used before initialization");
    }
}

bool scoop_gc_stress_move_enabled(void) {
    require_arena();
    return stress_move;
}

bool pointer_block_index(const void *pointer, uint32_t *index) {
    uintptr_t address = (uintptr_t)pointer;
    if (!arena_ready || address < arena_base ||
        address >= (uintptr_t)arena_end) {
        return false;
    }
    *index = (uint32_t)((address - arena_base) / GC_BLOCK_SIZE);
    return true;
}

bool bit_test(const uint64_t *bits, size_t index) {
    return ((bits[index / 64] >> (index % 64)) & UINT64_C(1)) != 0;
}

void bit_set(uint64_t *bits, size_t index) {
    bits[index / 64] |= UINT64_C(1) << (index % 64);
}

void bit_clear(uint64_t *bits, size_t index) {
    bits[index / 64] &= ~(UINT64_C(1) << (index % 64));
}

static void ensure_small_metadata(ScoopGcBlockMeta *block) {
    if (block->starts == NULL) {
        block->starts = calloc(GC_BITMAP_WORDS, sizeof(uint64_t));
        block->marks = calloc(GC_BITMAP_WORDS, sizeof(uint64_t));
        block->pins = calloc(GC_BITMAP_WORDS, sizeof(uint64_t));
        block->scanned = calloc(GC_BITMAP_WORDS, sizeof(uint64_t));
        block->line_occupied =
            calloc(GC_LINE_BITMAP_WORDS, sizeof(uint64_t));
        block->line_live =
            calloc(GC_LINE_BITMAP_WORDS, sizeof(uint64_t));
        block->size_units = calloc(GC_WORDS_PER_BLOCK, sizeof(uint8_t));
        if (block->starts == NULL || block->marks == NULL ||
            block->pins == NULL || block->scanned == NULL ||
            block->line_occupied == NULL || block->line_live == NULL ||
            block->size_units == NULL) {
            heap_fatal("out of memory allocating small-block side metadata");
        }
    }
}

static void reset_small_metadata(ScoopGcBlockMeta *block) {
    ensure_small_metadata(block);
    memset(block->starts, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->pins, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->line_occupied, 0,
           GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->line_live, 0,
           GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->size_units, 0, GC_WORDS_PER_BLOCK * sizeof(uint8_t));
    free(block->forwarding);
    block->forwarding = NULL;
}

void free_span_insert(uint32_t first_block, uint32_t block_count) {
    if (block_count == 0 || first_block > GC_BLOCK_COUNT - block_count) {
        heap_fatal("invalid free arena span");
    }
    ScoopGcFreeSpan *previous = NULL;
    ScoopGcFreeSpan *current = free_spans;
    while (current != NULL && current->first_block < first_block) {
        previous = current;
        current = current->next;
    }
    if ((previous != NULL &&
         previous->first_block + previous->block_count > first_block) ||
        (current != NULL && first_block + block_count > current->first_block)) {
        heap_fatal("overlapping free arena spans");
    }
    if (previous != NULL &&
        previous->first_block + previous->block_count == first_block) {
        previous->block_count += block_count;
        if (current != NULL &&
            previous->first_block + previous->block_count ==
                current->first_block) {
            previous->block_count += current->block_count;
            previous->next = current->next;
            free(current);
        }
        return;
    }
    if (current != NULL && first_block + block_count == current->first_block) {
        current->first_block = first_block;
        current->block_count += block_count;
        return;
    }
    ScoopGcFreeSpan *inserted = malloc(sizeof *inserted);
    if (inserted == NULL) {
        heap_fatal("out of memory recording a free arena span");
    }
    *inserted = (ScoopGcFreeSpan){
        .next = current,
        .first_block = first_block,
        .block_count = block_count,
    };
    if (previous == NULL) {
        free_spans = inserted;
    } else {
        previous->next = inserted;
    }
}

static bool arena_carve(uint32_t block_count, uint32_t *first_block) {
    ScoopGcFreeSpan *previous = NULL;
    for (ScoopGcFreeSpan *span = free_spans; span != NULL;
         span = span->next) {
        if (span->block_count >= block_count) {
            *first_block = span->first_block;
            span->first_block += block_count;
            span->block_count -= block_count;
            if (span->block_count == 0) {
                if (previous == NULL) {
                    free_spans = span->next;
                } else {
                    previous->next = span->next;
                }
                free(span);
            }
            return true;
        }
        previous = span;
    }
    if (block_count <= GC_BLOCK_COUNT - arena_next_block) {
        *first_block = arena_next_block;
        arena_next_block += block_count;
        return true;
    }
    return false;
}

uint32_t activate_small_block(ScoopGcBlockState state) {
    uint32_t index;
    if (!arena_carve(1, &index)) {
        return UINT32_MAX;
    }
    ScoopGcBlockMeta *block = &blocks[index];
    if (block->state != SCOOP_BLOCK_NEVER_USED &&
        block->state != SCOOP_BLOCK_FREE) {
        heap_fatal("arena allocator selected an active block");
    }
    reset_small_metadata(block);
    block->state = state;
    block->kind = SCOOP_BLOCK_KIND_SMALL;
    block->span_blocks = 1;
    block->owner_block = index;
    block->exact_size = 0;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    block->large_forwarding = NULL;
    block->large_published = false;
    block->large_marked = false;
    block->large_pinned = false;
    block->large_scanned = false;
    committed_bytes += GC_BLOCK_SIZE;
    active_block_heads++;
    return index;
}

uint32_t activate_large_block(size_t exact_size,
                              ScoopGcBlockState state) {
    if (exact_size > GC_ARENA_SIZE - GC_LINE_SIZE) {
        heap_fatal("large object size overflows its block span");
    }
    size_t span_size = GC_LINE_SIZE + exact_size;
    size_t rounded =
        (span_size + GC_BLOCK_SIZE - 1) & ~(GC_BLOCK_SIZE - 1);
    uint32_t span_blocks = (uint32_t)(rounded / GC_BLOCK_SIZE);
    uint32_t index;
    if (!arena_carve(span_blocks, &index)) {
        return UINT32_MAX;
    }
    ScoopGcBlockMeta *head = &blocks[index];
    if (head->state != SCOOP_BLOCK_NEVER_USED &&
        head->state != SCOOP_BLOCK_FREE) {
        heap_fatal("arena allocator selected an active large block");
    }
    free(head->forwarding);
    head->forwarding = NULL;
    head->state = state;
    head->kind = SCOOP_BLOCK_KIND_LARGE;
    head->span_blocks = span_blocks;
    head->owner_block = index;
    head->exact_size = exact_size;
    head->live_bytes = 0;
    head->movable_live_bytes = 0;
    head->large_forwarding = NULL;
    head->large_published = false;
    head->large_marked = false;
    head->large_pinned = false;
    head->large_scanned = false;
    for (uint32_t offset = 1; offset < span_blocks; offset++) {
        ScoopGcBlockMeta *tail = &blocks[index + offset];
        if (tail->state != SCOOP_BLOCK_NEVER_USED &&
            tail->state != SCOOP_BLOCK_FREE) {
            heap_fatal("large object span overlaps an active block");
        }
        free(tail->forwarding);
        tail->forwarding = NULL;
        tail->state = SCOOP_BLOCK_LARGE_TAIL;
        tail->kind = SCOOP_BLOCK_KIND_NONE;
        tail->span_blocks = 0;
        tail->owner_block = index;
    }
    committed_bytes += rounded;
    active_block_heads++;
    return index;
}

static size_t object_word_index(uint32_t block_index, const void *object) {
    uintptr_t offset = (uintptr_t)object - (uintptr_t)block_base(block_index);
    return (size_t)(offset / sizeof(uint64_t));
}

bool object_meta(const void *object, uint32_t *block_index,
                 size_t *word_index) {
    uintptr_t address = (uintptr_t)object;
    if ((address & (sizeof(void *) - 1)) != 0 ||
        !pointer_block_index(object, block_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[*block_index];
    if (!active_head(block)) {
        return false;
    }
    uintptr_t base = (uintptr_t)block_base(*block_index);
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (!block->large_published || address != base + GC_LINE_SIZE) {
            return false;
        }
        *word_index = GC_LINE_SIZE / sizeof(uint64_t);
        return true;
    }
    if (block->kind != SCOOP_BLOCK_KIND_SMALL ||
        address < base + GC_LINE_SIZE) {
        return false;
    }
    *word_index = object_word_index(*block_index, object);
    uint64_t starts = __atomic_load_n(
        &block->starts[*word_index / 64], __ATOMIC_ACQUIRE);
    return ((starts >> (*word_index % 64)) & UINT64_C(1)) != 0;
}

bool scoop_gc_is_object_start_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    return object_meta(object, &block_index, &word_index);
}

size_t scoop_gc_object_size_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("exact size requested for a non-object address");
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        return block->exact_size;
    }
    uint8_t units = block->size_units[word_index];
    if (units == 0) {
        heap_fatal("published small object has no exact allocation size");
    }
    return (size_t)units * sizeof(uint64_t);
}

void record_small_object(uint32_t block_index, void *object,
                         size_t exact_size, bool marked) {
    ScoopGcBlockMeta *block = &blocks[block_index];
    if (block->kind != SCOOP_BLOCK_KIND_SMALL || exact_size == 0 ||
        exact_size > GC_SMALL_MAX || exact_size % sizeof(uint64_t) != 0) {
        heap_fatal("invalid small-object metadata publication");
    }
    size_t word = object_word_index(block_index, object);
    uintptr_t base = (uintptr_t)block_base(block_index);
    uintptr_t address = (uintptr_t)object;
    size_t line = (size_t)((address - base) / GC_LINE_SIZE);
    if (line == 0 || word >= GC_WORDS_PER_BLOCK ||
        address + exact_size > base + GC_BLOCK_SIZE ||
        (address / GC_LINE_SIZE) !=
            ((address + exact_size - 1) / GC_LINE_SIZE) ||
        bit_test(block->starts, word)) {
        heap_fatal("small object overlaps invalid storage");
    }
    block->size_units[word] = (uint8_t)(exact_size / sizeof(uint64_t));
    bit_set(block->line_occupied, line);
    if (marked) {
        bit_set(block->marks, word);
        bit_set(block->line_live, line);
        block->live_bytes += exact_size;
        block->movable_live_bytes += exact_size;
    }
    (void)__atomic_fetch_or(&block->starts[word / 64],
                            UINT64_C(1) << (word % 64),
                            __ATOMIC_RELEASE);
}

void publish_large_object(uint32_t block_index, bool marked) {
    ScoopGcBlockMeta *block = &blocks[block_index];
    if (block->kind != SCOOP_BLOCK_KIND_LARGE || block->large_published ||
        block->exact_size == 0) {
        heap_fatal("invalid large-object metadata publication");
    }
    block->large_marked = marked;
    if (marked) {
        block->live_bytes = block->exact_size;
        block->movable_live_bytes = block->exact_size;
    }
    __atomic_store_n(&block->large_published, true, __ATOMIC_RELEASE);
}

bool scoop_gc_update_pin_locked(const void *object, bool pinned) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("pin state changed for a non-object address");
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    bool previous;
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        previous = block->large_pinned;
        block->large_pinned = pinned;
    } else {
        previous = bit_test(block->pins, word_index);
        if (pinned) {
            bit_set(block->pins, word_index);
        } else {
            bit_clear(block->pins, word_index);
        }
    }
    ScoopObjectHeader *header = (ScoopObjectHeader *)object;
    if (pinned) {
        (void)__atomic_fetch_or(&header->gc_word, GC_PIN_BIT,
                                __ATOMIC_ACQ_REL);
    } else {
        (void)__atomic_fetch_and(&header->gc_word, ~GC_PIN_BIT,
                                 __ATOMIC_ACQ_REL);
    }
    return previous;
}

bool object_pinned(const ScoopGcBlockMeta *block, size_t word) {
    return block->kind == SCOOP_BLOCK_KIND_LARGE
               ? block->large_pinned
               : bit_test(block->pins, word);
}

bool object_marked(const ScoopGcBlockMeta *block, size_t word) {
    return block->kind == SCOOP_BLOCK_KIND_LARGE
               ? block->large_marked
               : bit_test(block->marks, word);
}

void scoop_gc_heap_begin_collection_locked(void) {
    require_arena();
    if (collection_active) {
        heap_fatal("nested heap collection");
    }
    free_run_nodes();
    evacuation_block = UINT32_MAX;
    evacuation_cursor = NULL;
    evacuation_limit = NULL;
    moved_objects = 0;
    for (uint32_t index = 0; index < arena_next_block; index++) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block)) {
            continue;
        }
        if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE ||
            block->state == SCOOP_BLOCK_EVACUATION_TARGET) {
            heap_fatal("unfinished evacuation state at collection start");
        }
        block->live_bytes = 0;
        block->movable_live_bytes = 0;
        block->large_forwarding = NULL;
        block->large_marked = false;
        block->large_scanned = false;
        free(block->forwarding);
        block->forwarding = NULL;
        if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
            memset(block->marks, 0,
                   GC_BITMAP_WORDS * sizeof(uint64_t));
            memset(block->scanned, 0,
                   GC_BITMAP_WORDS * sizeof(uint64_t));
            memset(block->line_live, 0,
                   GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
        }
    }
    collection_active = true;
}

bool scoop_gc_mark_object_locked(const void *object) {
    if (!collection_active) {
        heap_fatal("object marked outside collection");
    }
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("mark requested for a non-object address");
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    if (object_marked(block, word_index)) {
        return false;
    }
    size_t size = scoop_gc_object_size_locked(object);
    bool pinned = object_pinned(block, word_index);
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        block->large_marked = true;
    } else {
        bit_set(block->marks, word_index);
        uintptr_t base = (uintptr_t)block_base(block_index);
        uintptr_t first = (uintptr_t)object - base;
        uintptr_t last = first + size - 1;
        for (size_t line = first / GC_LINE_SIZE;
             line <= last / GC_LINE_SIZE; line++) {
            bit_set(block->line_live, line);
        }
    }
    block->live_bytes += size;
    if (!pinned) {
        block->movable_live_bytes += size;
    }
    return true;
}

uint64_t scoop_rt_gc_stats(void) {
    return atomic_load_explicit(&live_objects, memory_order_acquire);
}

uint64_t scoop_rt_gc_debug_last_moved_count(void) {
    return atomic_load_explicit(&last_moved_objects,
                                memory_order_acquire);
}

uint64_t scoop_rt_gc_debug_block_count(void) {
    lock_heap();
    uint64_t count = active_block_heads;
    unlock_heap();
    return count;
}

uintptr_t scoop_rt_gc_debug_arena_base(void) {
    return arena_base;
}

bool scoop_rt_gc_debug_is_allocated(const void *object) {
    lock_heap();
    bool allocated = scoop_gc_is_object_start_locked(object);
    unlock_heap();
    return allocated;
}

uint64_t scoop_rt_gc_debug_allocation_size(const void *object) {
    lock_heap();
    uint64_t size = (uint64_t)scoop_gc_object_size_locked(object);
    unlock_heap();
    return size;
}
