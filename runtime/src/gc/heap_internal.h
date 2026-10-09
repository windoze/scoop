#ifndef SCOOP_GC_HEAP_INTERNAL_H
#define SCOOP_GC_HEAP_INTERNAL_H

#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "scoop_rt.h"

#define GC_BLOCK_SIZE ((size_t)32768)
#define GC_LINE_SIZE ((size_t)128)
#define GC_LINES_PER_BLOCK (GC_BLOCK_SIZE / GC_LINE_SIZE)
#define GC_WORDS_PER_BLOCK (GC_BLOCK_SIZE / sizeof(uint64_t))
#define GC_BITMAP_WORDS (GC_WORDS_PER_BLOCK / 64)
#define GC_LINE_BITMAP_WORDS (GC_LINES_PER_BLOCK / 64)
#define GC_REGULAR_MAX (GC_BLOCK_SIZE - GC_LINE_SIZE)
#define GC_INITIAL_THRESHOLD ((size_t)16 << 20)
#define GC_NURSERY_CAPACITY ((size_t)1 << 20)
#define GC_REGION_SIZE ((size_t)16 << 20)
#define GC_REGION_BLOCKS (GC_REGION_SIZE / GC_BLOCK_SIZE)
#define GC_CHUNK_SHIFT 16
#define GC_CHUNK_SIZE ((size_t)1 << GC_CHUNK_SHIFT)
#define GC_RADIX_BITS 12
#define GC_RADIX_ENTRIES ((size_t)1 << GC_RADIX_BITS)
#define GC_CARD_SHIFT 9
#define GC_PIN_BIT UINT64_C(2)
#define GC_RELEASE_READY_BIT UINT64_C(4)
#define GC_POISON_BYTE ((unsigned char)0xA5)

typedef enum ScoopGcBlockState {
    SCOOP_BLOCK_NEVER_USED,
    SCOOP_BLOCK_FREE,
    SCOOP_BLOCK_MUTATOR,
    SCOOP_BLOCK_EVACUATION_SOURCE,
    SCOOP_BLOCK_EVACUATION_TARGET,
    SCOOP_BLOCK_PINNED_PARTIAL,
    SCOOP_BLOCK_QUARANTINED,
} ScoopGcBlockState;

typedef enum ScoopGcBlockKind {
    SCOOP_BLOCK_KIND_NONE,
    SCOOP_BLOCK_KIND_SMALL,
    SCOOP_BLOCK_KIND_LARGE,
} ScoopGcBlockKind;

typedef enum ScoopGcGeneration {
    SCOOP_GC_YOUNG,
    SCOOP_GC_OLD,
} ScoopGcGeneration;

typedef struct ScoopGcRegion ScoopGcRegion;

typedef struct ScoopGcBlockMeta {
    ScoopGcRegion *region;
    struct ScoopGcBlockMeta *next_free;
    uint16_t index;
    ScoopGcBlockState state;
    ScoopGcBlockKind kind;
    ScoopGcGeneration generation;
    uint64_t *starts;
    uint64_t *marks;
    uint64_t *pins;
    uint64_t *scanned;
    uint64_t *line_occupied;
    uint64_t *line_live;
    uint16_t *size_units;
    void **forwarding;
    size_t exact_size;
    size_t live_bytes;
    size_t movable_live_bytes;
    void *large_forwarding;
    bool large_published;
    bool large_marked;
    bool large_pinned;
    bool large_scanned;
} ScoopGcBlockMeta;

struct ScoopGcRegion {
    /* Immutable generated-code prefix, published through the page map. */
    uintptr_t base;
    size_t size;
    unsigned char *cards;
    ScoopGcRegion *next;
    ScoopGcBlockMeta *blocks;
    uint16_t next_block;
    bool large;
};

_Static_assert(sizeof(uintptr_t) == 8 && GC_CHUNK_SHIFT + 4 * GC_RADIX_BITS == 64,
               "page map must cover every target address bit");
_Static_assert(offsetof(ScoopGcRegion, base) == 0 && offsetof(ScoopGcRegion, size) == 8 &&
                   offsetof(ScoopGcRegion, cards) == 16,
               "generated region metadata prefix drifted");

typedef struct ScoopGcFreeRun {
    struct ScoopGcFreeRun *next;
    ScoopGcBlockMeta *block;
    uint16_t first_line;
    uint16_t line_count;
} ScoopGcFreeRun;

typedef struct ScoopGcHeapState {
    pthread_mutex_t lock;
    ScoopGcRegion *regions;
    ScoopGcRegion *allocation_region;
    ScoopGcBlockMeta *free_blocks;
    ScoopGcFreeRun *free_runs;
    size_t committed_bytes;
    size_t collection_threshold;
    uint64_t active_block_heads;
    uint64_t collected_live_objects;
    uint64_t allocation_objects_at_collection;
    uint64_t nursery_objects_at_collection;
    _Atomic(uint64_t) last_moved_objects;
    bool ready;
    bool stress_move;
    bool stress_minor;
    bool full_only;
    bool print_metrics;
    bool collection_active;
    size_t nursery_bytes;
    ScoopGcMetrics metrics;
    uint64_t copied_bytes;
    uint64_t minor_pause_ns;
    uint64_t full_pause_ns;
    uint64_t pause_buckets[8];
    char *old_cursor;
    char *old_limit;
    ScoopGcBlockMeta *evacuation_block;
    char *evacuation_cursor;
    char *evacuation_limit;
    uint64_t moved_objects;
} ScoopGcHeapState;

extern ScoopGcHeapState scoop_gc_heap_state;

/* Short aliases are confined to the private heap implementation modules. */
#define heap_lock (scoop_gc_heap_state.lock)
#define free_runs (scoop_gc_heap_state.free_runs)
#define committed_bytes (scoop_gc_heap_state.committed_bytes)
#define collection_threshold (scoop_gc_heap_state.collection_threshold)
#define active_block_heads (scoop_gc_heap_state.active_block_heads)
#define last_moved_objects (scoop_gc_heap_state.last_moved_objects)
#define stress_move (scoop_gc_heap_state.stress_move)
#define collection_active (scoop_gc_heap_state.collection_active)
#define evacuation_block (scoop_gc_heap_state.evacuation_block)
#define evacuation_cursor (scoop_gc_heap_state.evacuation_cursor)
#define evacuation_limit (scoop_gc_heap_state.evacuation_limit)
#define moved_objects (scoop_gc_heap_state.moved_objects)

#define heap_fatal scoop_heap_fatal
#define lock_heap scoop_heap_lock
#define unlock_heap scoop_heap_unlock
#define block_base scoop_heap_block_base
#define active_head scoop_heap_active_head
#define require_heap scoop_heap_require_ready
#define free_run_nodes scoop_heap_free_run_nodes
#define pointer_block scoop_heap_pointer_block
#define bit_test scoop_heap_bit_test
#define bit_set scoop_heap_bit_set
#define bit_clear scoop_heap_bit_clear
#define activate_small_block scoop_heap_activate_small_block
#define activate_large_block scoop_heap_activate_large_block
#define object_meta scoop_heap_object_meta
#define record_small_object scoop_heap_record_small_object
#define publish_large_object scoop_heap_publish_large_object
#define object_pinned scoop_heap_object_pinned
#define object_marked scoop_heap_object_marked

_Noreturn void scoop_heap_fatal(const char *message);
void scoop_heap_lock(void);
void scoop_heap_unlock(void);
void *scoop_heap_block_base(const ScoopGcBlockMeta *block);
size_t scoop_heap_block_bytes(const ScoopGcBlockMeta *block);
bool scoop_heap_active_head(const ScoopGcBlockMeta *block);
void scoop_heap_require_ready(void);
void scoop_heap_free_run_nodes(void);
ScoopGcBlockMeta *scoop_heap_first_block(void);
ScoopGcBlockMeta *scoop_heap_next_block(const ScoopGcBlockMeta *block);
ScoopGcBlockMeta *scoop_heap_pointer_block(const void *pointer);
bool scoop_heap_bit_test(const uint64_t *bits, size_t index);
void scoop_heap_bit_set(uint64_t *bits, size_t index);
void scoop_heap_bit_clear(uint64_t *bits, size_t index);
ScoopGcBlockMeta *scoop_heap_activate_small_block(ScoopGcBlockState state);
ScoopGcBlockMeta *scoop_heap_activate_large_block(size_t exact_size, ScoopGcBlockState state);
bool scoop_heap_object_meta(const void *object, ScoopGcBlockMeta **block, size_t *word_index);
void scoop_heap_record_small_object(ScoopGcBlockMeta *block, void *object, size_t exact_size,
                                    bool marked);
void scoop_heap_publish_large_object(ScoopGcBlockMeta *block, bool marked);
void scoop_heap_release_block(ScoopGcBlockMeta *block);
void scoop_heap_quarantine_block(ScoopGcBlockMeta *block);
void *scoop_heap_bump(char **cursor, char *limit, size_t size, size_t alignment);
bool scoop_heap_take_free_run(size_t size, char **cursor, char **limit);
bool scoop_heap_object_pinned(const ScoopGcBlockMeta *block, size_t word);
bool scoop_heap_object_marked(const ScoopGcBlockMeta *block, size_t word);

/* Mapping mutation requires the heap lock; removal additionally requires STW. */
size_t scoop_heap_large_mapping_size(size_t exact_size);
ScoopGcRegion *scoop_heap_region_create(size_t size, bool large);
void scoop_heap_region_destroy(ScoopGcRegion *region);
void scoop_heap_release_empty_large_regions(void);
ScoopGcRegion *scoop_heap_region_for_address(uintptr_t address);
void scoop_heap_page_map_publish(ScoopGcRegion *region);
void scoop_heap_page_map_remove(const ScoopGcRegion *region);
void scoop_heap_page_map_prune(void);

#endif /* SCOOP_GC_HEAP_INTERNAL_H */
