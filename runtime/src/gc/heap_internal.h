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
#define GC_SMALL_MAX (GC_LINE_SIZE / 2)
#define GC_INITIAL_THRESHOLD ((size_t)16 << 20)
#define GC_ARENA_SIZE ((size_t)1 << 30)
#define GC_BLOCK_COUNT (GC_ARENA_SIZE / GC_BLOCK_SIZE)
#define GC_ARENA_HINT ((uintptr_t)0x100000000)
#define GC_CARD_SHIFT 9
#define GC_CARD_TABLE_SIZE ((size_t)4 << 20)
#define GC_PIN_BIT UINT64_C(2)
#define GC_POISON_BYTE ((unsigned char)0xA5)

typedef enum ScoopGcBlockState {
    SCOOP_BLOCK_NEVER_USED,
    SCOOP_BLOCK_FREE,
    SCOOP_BLOCK_MUTATOR,
    SCOOP_BLOCK_EVACUATION_SOURCE,
    SCOOP_BLOCK_EVACUATION_TARGET,
    SCOOP_BLOCK_PINNED_PARTIAL,
    SCOOP_BLOCK_LARGE_TAIL,
    SCOOP_BLOCK_QUARANTINED,
} ScoopGcBlockState;

typedef enum ScoopGcBlockKind {
    SCOOP_BLOCK_KIND_NONE,
    SCOOP_BLOCK_KIND_SMALL,
    SCOOP_BLOCK_KIND_LARGE,
} ScoopGcBlockKind;

typedef struct ScoopGcBlockMeta {
    ScoopGcBlockState state;
    ScoopGcBlockKind kind;
    uint32_t span_blocks;
    uint32_t owner_block;
    uint64_t *starts;
    uint64_t *marks;
    uint64_t *pins;
    uint64_t *scanned;
    uint64_t *line_occupied;
    uint64_t *line_live;
    uint8_t *size_units;
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

typedef struct ScoopGcFreeSpan {
    struct ScoopGcFreeSpan *next;
    uint32_t first_block;
    uint32_t block_count;
} ScoopGcFreeSpan;

typedef struct ScoopGcFreeRun {
    struct ScoopGcFreeRun *next;
    uint32_t block_index;
    uint16_t first_line;
    uint16_t line_count;
} ScoopGcFreeRun;

typedef struct ScoopGcHeapState {
    pthread_mutex_t lock;
    uintptr_t arena_base;
    char *arena_end;
    uint32_t arena_next_block;
    ScoopGcBlockMeta *blocks;
    ScoopGcFreeSpan *free_spans;
    ScoopGcFreeRun *free_runs;
    unsigned char *card_table_storage;
    size_t committed_bytes;
    size_t collection_threshold;
    uint64_t active_block_heads;
    _Atomic(uint64_t) live_objects;
    _Atomic(uint64_t) last_moved_objects;
    bool arena_ready;
    bool stress_move;
    bool collection_active;
    uint32_t evacuation_block;
    char *evacuation_cursor;
    char *evacuation_limit;
    uint64_t moved_objects;
} ScoopGcHeapState;

extern ScoopGcHeapState scoop_gc_heap_state;

/* Short aliases are confined to the private heap implementation modules. */
#define heap_lock (scoop_gc_heap_state.lock)
#define arena_base (scoop_gc_heap_state.arena_base)
#define arena_end (scoop_gc_heap_state.arena_end)
#define arena_next_block (scoop_gc_heap_state.arena_next_block)
#define blocks (scoop_gc_heap_state.blocks)
#define free_spans (scoop_gc_heap_state.free_spans)
#define free_runs (scoop_gc_heap_state.free_runs)
#define card_table_storage (scoop_gc_heap_state.card_table_storage)
#define committed_bytes (scoop_gc_heap_state.committed_bytes)
#define collection_threshold (scoop_gc_heap_state.collection_threshold)
#define active_block_heads (scoop_gc_heap_state.active_block_heads)
#define live_objects (scoop_gc_heap_state.live_objects)
#define last_moved_objects (scoop_gc_heap_state.last_moved_objects)
#define arena_ready (scoop_gc_heap_state.arena_ready)
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
#define free_run_nodes scoop_heap_free_run_nodes
#define free_span_insert scoop_heap_free_span_insert
#define pointer_block_index scoop_heap_pointer_block_index
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
void *scoop_heap_block_base(uint32_t index);
bool scoop_heap_active_head(const ScoopGcBlockMeta *block);
void scoop_heap_free_run_nodes(void);
void scoop_heap_free_span_insert(uint32_t first_block,
                                 uint32_t block_count);
bool scoop_heap_pointer_block_index(const void *pointer, uint32_t *index);
bool scoop_heap_bit_test(const uint64_t *bits, size_t index);
void scoop_heap_bit_set(uint64_t *bits, size_t index);
void scoop_heap_bit_clear(uint64_t *bits, size_t index);
uint32_t scoop_heap_activate_small_block(ScoopGcBlockState state);
uint32_t scoop_heap_activate_large_block(size_t exact_size,
                                         ScoopGcBlockState state);
bool scoop_heap_object_meta(const void *object, uint32_t *block_index,
                            size_t *word_index);
void scoop_heap_record_small_object(uint32_t block_index, void *object,
                                    size_t exact_size, bool marked);
void scoop_heap_publish_large_object(uint32_t block_index, bool marked);
bool scoop_heap_object_pinned(const ScoopGcBlockMeta *block, size_t word);
bool scoop_heap_object_marked(const ScoopGcBlockMeta *block, size_t word);

#endif /* SCOOP_GC_HEAP_INTERNAL_H */
