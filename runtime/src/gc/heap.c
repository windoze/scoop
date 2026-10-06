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

void scoop_rt_gc_write_barrier(const void *destination, size_t bytes) {
    if (bytes == 0) {
        return;
    }
    uintptr_t address = (uintptr_t)destination;
    if (!arena_ready || address < arena_base || address >= (uintptr_t)arena_end ||
        bytes > (uintptr_t)arena_end - address) {
        heap_fatal("write barrier range lies outside the GC arena");
    }
    size_t first = (address - arena_base) >> GC_CARD_SHIFT;
    size_t last = (address - arena_base + bytes - 1) >> GC_CARD_SHIFT;
    for (size_t card = first; card <= last; card++) {
        (void)__atomic_fetch_or(&card_table_storage[card], 1, __ATOMIC_RELAXED);
    }
}
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
    const char *minor = getenv("SCOOP_GC_STRESS_MINOR");
    scoop_gc_heap_state.stress_minor = minor != NULL && strcmp(minor, "1") == 0;
    const char *metrics = getenv("SCOOP_GC_STATS");
    scoop_gc_heap_state.print_metrics = metrics != NULL && strcmp(metrics, "1") == 0;
    arena_init();
}

void require_arena(void) {
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
        block->size_units = calloc(GC_WORDS_PER_BLOCK, sizeof(uint16_t));
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
    memset(block->size_units, 0, GC_WORDS_PER_BLOCK * sizeof(uint16_t));
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
    block->generation = SCOOP_GC_OLD;
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
    head->generation = SCOOP_GC_OLD;
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
