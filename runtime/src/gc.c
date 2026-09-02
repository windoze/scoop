/* Immix heap storage (runtime spec sections 3-4).
 *
 * This module owns the arena, block/object-start metadata, allocation, mark
 * bits and reclamation. `gc/collector.c` owns graph traversal and thread-root
 * enumeration; `gc/roots.c` owns every non-stack root source. Their internal
 * API requires the global heap-before-roots lock order.
 *
 * The M13 heap remains single-generation and non-moving while the M15 module
 * boundaries are established: 32KB blocks, 128B lines, owner-only TLABs and
 * dedicated large-object blocks. Moving evacuation replaces this module's
 * mark/sweep operations without leaking block internals into the collector.
 */
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#include "scoop_rt.h"
#include "gc/gc_internal.h"
#include "thread.h"

#ifndef MAP_ANON
#define MAP_ANON MAP_ANONYMOUS /* POSIX name; macOS/BSD use MAP_ANON */
#endif

/* --- configuration (DESIGN 2.1) ------------------------------------ */

#define GC_BLOCK_SIZE ((size_t)32768) /* 32KB, also the block alignment */
#define GC_LINE_SIZE ((size_t)128)
#define GC_LINES_PER_BLOCK (GC_BLOCK_SIZE / GC_LINE_SIZE) /* 256 */
/* Objects strictly larger than half a line are "large" and own a whole
 * block mapping (DESIGN 2.1). With the 16-byte header, objects up to
 * 64B stay small; they always fit a single line. */
#define GC_SMALL_MAX (GC_LINE_SIZE / 2) /* 64 */
/* Slow-path collection trigger: heap bytes committed from the OS. v1
 * takes the DESIGN's second option with a floor — fixed 16MB floor,
 * raised to 2x the committed bytes after each cycle (a pure fixed
 * threshold thrashes once live data hovers at the limit). */
#define GC_INITIAL_THRESHOLD ((size_t)16 << 20)

/* Heap arena: one contiguous reservation the blocks are carved from.
 * Fixed 1 GiB in v1 — exhaustion aborts; growing the arena is in the
 * backlog. The 4 GiB start is a hint only; if the OS maps elsewhere
 * the recorded base is used (the card-table bias adapts). */
#define GC_ARENA_SIZE ((size_t)1 << 30) /* 1 GiB */
#define GC_ARENA_HINT ((uintptr_t)0x100000000) /* 4 GiB */
/* Card table backing store (spec 3.6): one byte per 512B card, 4 MiB
 * covers a 2 GiB window starting at the arena base — the 1 GiB arena
 * always fits. */
#define GC_CARD_SHIFT 9
#define GC_CARD_TABLE_SIZE ((size_t)4 << 20)

/* gc_word bits (ScoopObjectHeader, runtime spec 2.1). */
#define GC_MARK_BIT UINT64_C(1) /* parity of the cycle that marked it */
#define GC_SMALL 0
#define GC_LARGE 1

/* --- arena & card table ----------------------------------------------- */

/* The heap's blocks are carved out of ONE contiguous address-space
 * reservation instead of per-block aligned_alloc: heap addresses then
 * span a known window [gc_arena_base, gc_arena_base + GC_ARENA_SIZE),
 * which is what makes the compiler's card-mark formula
 * `scoop_gc_card_table[addr >> 9]` indexable into a bounded table.
 * (The previous per-block aligned_alloc scattered blocks across the
 * address space; `addr >> 9` overran the table and the store hit
 * unmapped pages — the integration SIGSEGV.) */

/* Write-barrier card table (spec 3.6): the compiler atomically ORs 1 into
 * `scoop_gc_card_table[addr >> 9]` after heap stores. This symbol
 * is a POINTER VARIABLE (generated code loads it, then GEPs): the
 * runtime pre-biases it by `arena_base >> 9`, so the formula lands in
 * the backing store as `real[(addr - arena_base) >> 9]` — exact
 * because the arena base is 512B-aligned (32KB). The barrier is only
 * emitted for heap stores, and heap addresses live in the arena, so
 * the index is always in bounds. v1's collector ignores the cards;
 * the table exists so generational collection can consume them later
 * without changing generated code. */
unsigned char *scoop_gc_card_table;

static uintptr_t gc_arena_base; /* 32KB-aligned base of the usable arena */
static char *gc_arena_next; /* bump cursor for carving blocks */
static char *gc_arena_end;
static int gc_arena_ready;

static _Noreturn void gc_fatal(const char *message);

static pthread_mutex_t gc_heap_lock = PTHREAD_MUTEX_INITIALIZER;

static void gc_lock(pthread_mutex_t *lock, const char *message) {
    if (pthread_mutex_lock(lock) != 0) {
        gc_fatal(message);
    }
}

static void gc_unlock(pthread_mutex_t *lock, const char *message) {
    if (pthread_mutex_unlock(lock) != 0) {
        gc_fatal(message);
    }
}

void scoop_gc_heap_lock(void) {
    gc_lock(&gc_heap_lock, "failed to lock the heap");
}

void scoop_gc_heap_unlock(void) {
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
}

static void gc_arena_init(void) {
    /* Reserve the arena plus one block of alignment slack; the usable
     * base is aligned up to 32KB inside the mapping so block headers
     * stay discoverable by address masking. */
    size_t reserve = GC_ARENA_SIZE + GC_BLOCK_SIZE;
    void *mem = mmap((void *)GC_ARENA_HINT, reserve, PROT_READ | PROT_WRITE,
                     MAP_PRIVATE | MAP_ANON, -1, 0);
    if (mem == MAP_FAILED) {
        gc_fatal("out of memory reserving the heap arena");
    }
    /* The hint is best-effort: whatever base the OS picked is fine. */
    gc_arena_base = ((uintptr_t)mem + GC_BLOCK_SIZE - 1) & ~(uintptr_t)(GC_BLOCK_SIZE - 1);
    gc_arena_next = (char *)gc_arena_base;
    gc_arena_end = (char *)gc_arena_base + GC_ARENA_SIZE;
    /* Pre-bias the card table. The biased pointer points outside its
     * backing object between the arena base and the table — computed
     * via uintptr_t so only the final, in-bounds access is a pointer
     * dereference. */
    unsigned char *real = calloc(1, GC_CARD_TABLE_SIZE);
    if (real == NULL) {
        gc_fatal("out of memory allocating the card table");
    }
    scoop_gc_card_table =
        (unsigned char *)((uintptr_t)real - (gc_arena_base >> GC_CARD_SHIFT));
    gc_arena_ready = 1;
}

static void gc_arena_ensure(void) {
    if (!gc_arena_ready) {
        gc_arena_init();
    }
}

/* Free blocks go back to this list (first-fit with splitting at 32KB
 * granularity; adjacent runs are not coalesced in v1 — the 32KB
 * granularity keeps fragmentation bounded, coalescing is backlog).
 * Entries live in the freed block's own memory. */
typedef struct ScoopGcFreeBlock {
    struct ScoopGcFreeBlock *next;
    size_t size;
} ScoopGcFreeBlock;

static ScoopGcFreeBlock *gc_arena_free_blocks;

static void gc_arena_return(void *base, size_t map_size) {
    ScoopGcFreeBlock *entry = base;
    entry->next = gc_arena_free_blocks;
    entry->size = map_size;
    gc_arena_free_blocks = entry;
}

/* Carve `map_size` (a multiple of 32KB) out of the arena. The caller holds
 * the heap lock. Exhaustion is reported without collecting because a GC
 * request must never be made while a metadata lock is held. */
static void *gc_arena_carve(size_t map_size) {
    gc_arena_ensure();
    ScoopGcFreeBlock *prev = NULL;
    for (ScoopGcFreeBlock *cur = gc_arena_free_blocks; cur != NULL;
         cur = cur->next) {
        if (cur->size >= map_size) {
            if (cur->size > map_size) {
                /* Split: tail stays on the free list. */
                ScoopGcFreeBlock *tail = (ScoopGcFreeBlock *)((char *)cur + map_size);
                tail->next = cur->next;
                tail->size = cur->size - map_size;
                if (prev == NULL) {
                    gc_arena_free_blocks = tail;
                } else {
                    prev->next = tail;
                }
            } else if (prev == NULL) {
                gc_arena_free_blocks = cur->next;
            } else {
                prev->next = cur->next;
            }
            return cur;
        }
        prev = cur;
    }
    if (gc_arena_next + map_size <= gc_arena_end) {
        void *p = gc_arena_next;
        gc_arena_next += map_size;
        return p;
    }
    return NULL;
}

/* --- blocks --------------------------------------------------------- */

/* Per-block header, stored in the block's own line 0 (blocks are
 * 32KB-aligned, so the header is found by address masking). Line 0 is
 * never allocated from. */
typedef struct ScoopGcBlock {
    struct ScoopGcBlock *next; /* all-blocks list */
    /* Object-start bitmap, one bit per 8-byte word of the block
     * (4096 bits = 512B), side-allocated; small blocks only. It lets
     * the conservative stack scan validate candidate roots without
     * dereferencing arbitrary words. */
    uint64_t *start_bits;
    /* Live-line table, one bit per line; small blocks only. */
    uint64_t line_marks[GC_LINES_PER_BLOCK / 64];
    uint64_t map_size; /* bytes mapped from the OS (>= GC_BLOCK_SIZE) */
    uint32_t kind; /* GC_SMALL / GC_LARGE */
    uint32_t reserved;
} ScoopGcBlock;

_Static_assert(sizeof(ScoopGcBlock) <= GC_LINE_SIZE,
               "block header must fit into line 0");

/* Object of a large block: right after the header line. */
#define GC_LARGE_OBJECT_OFFSET GC_LINE_SIZE

static ScoopGcBlock *gc_blocks; /* all-blocks list head */
static size_t gc_committed; /* heap bytes currently mapped */
static size_t gc_threshold = GC_INITIAL_THRESHOLD;
static uint64_t gc_block_count;

/* Block-base hash set (open addressing, linear probing) for the
 * containment check. Keys are 32KB-aligned block base addresses, which
 * are also the ScoopGcBlock pointers. */
#define GC_HASH_EMPTY NULL
#define GC_HASH_TOMB ((ScoopGcBlock *)(uintptr_t)1)
static ScoopGcBlock **gc_table;
static size_t gc_table_cap; /* power of two */
static size_t gc_table_used; /* live + tombstone entries */

static _Noreturn void gc_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

static size_t gc_table_index(uintptr_t base) {
    /* base is 32KB-aligned; mix the address bits above the alignment. */
    uint64_t h = (uint64_t)(base >> 15) * UINT64_C(11400714819323198485);
    return (size_t)(h & (gc_table_cap - 1));
}

static void gc_table_put(ScoopGcBlock **table, size_t cap, ScoopGcBlock *block) {
    size_t i = (size_t)(((uint64_t)((uintptr_t)block >> 15) * UINT64_C(11400714819323198485)) &
                        (cap - 1));
    while (table[i] != GC_HASH_EMPTY) {
        i = (i + 1) & (cap - 1);
    }
    table[i] = block;
}

static void gc_table_grow(void) {
    size_t new_cap = gc_table_cap == 0 ? 64 : gc_table_cap * 2;
    ScoopGcBlock **new_table = calloc(new_cap, sizeof *new_table);
    if (new_table == NULL) {
        gc_fatal("out of memory growing the block table");
    }
    for (size_t i = 0; i < gc_table_cap; i++) {
        if (gc_table[i] != GC_HASH_EMPTY && gc_table[i] != GC_HASH_TOMB) {
            gc_table_put(new_table, new_cap, gc_table[i]);
        }
    }
    free(gc_table);
    gc_table = new_table;
    gc_table_cap = new_cap;
    gc_table_used = gc_block_count;
}

static ScoopGcBlock *gc_table_lookup(uintptr_t base) {
    if (gc_table_cap == 0) {
        return NULL;
    }
    size_t i = gc_table_index(base);
    for (;;) {
        ScoopGcBlock *entry = gc_table[i];
        if (entry == GC_HASH_EMPTY) {
            return NULL;
        }
        if (entry != GC_HASH_TOMB && (uintptr_t)entry == base) {
            return entry;
        }
        i = (i + 1) & (gc_table_cap - 1);
    }
}

static void gc_table_insert(ScoopGcBlock *block) {
    /* Grow at 70% load (tombstones included) to keep probes short. */
    if ((gc_table_used + 1) * 10 >= (gc_table_cap == 0 ? 1 : gc_table_cap) * 7) {
        gc_table_grow();
    }
    size_t i = gc_table_index((uintptr_t)block);
    for (;;) {
        ScoopGcBlock *entry = gc_table[i];
        if (entry == GC_HASH_EMPTY || entry == GC_HASH_TOMB) {
            gc_table[i] = block;
            gc_table_used++;
            return;
        }
        i = (i + 1) & (gc_table_cap - 1);
    }
}

static void gc_table_remove(uintptr_t base) {
    size_t i = gc_table_index(base);
    for (;;) {
        ScoopGcBlock *entry = gc_table[i];
        if (entry == GC_HASH_EMPTY) {
            return; /* unreachable for a live block */
        }
        if (entry != GC_HASH_TOMB && (uintptr_t)entry == base) {
            gc_table[i] = GC_HASH_TOMB;
            return;
        }
        i = (i + 1) & (gc_table_cap - 1);
    }
}

/* The block header of an address known to be inside a heap block. */
static ScoopGcBlock *gc_block_of(const void *p) {
    return (ScoopGcBlock *)((uintptr_t)p & ~(uintptr_t)(GC_BLOCK_SIZE - 1));
}

static ScoopGcBlock *gc_block_new(size_t map_size, uint32_t kind) {
    /* map_size is a multiple of the 32KB block size (callers round
     * up), carved out of the arena. */
    void *mem = gc_arena_carve(map_size);
    if (mem == NULL) {
        return NULL;
    }
    ScoopGcBlock *block = mem;
    block->start_bits = NULL;
    if (kind == GC_SMALL) {
        block->start_bits = calloc(GC_BLOCK_SIZE / 8 / 64, sizeof(uint64_t));
        if (block->start_bits == NULL) {
            gc_fatal("out of memory allocating the start bitmap");
        }
    }
    memset(block->line_marks, 0, sizeof block->line_marks);
    block->map_size = (uint64_t)map_size;
    block->kind = kind;
    block->reserved = 0;
    block->next = gc_blocks;
    gc_blocks = block;
    gc_table_insert(block);
    gc_committed += map_size;
    gc_block_count++;
    return block;
}

/* Unlink and return to the arena's free-block list (the arena is one
 * fixed reservation; blocks are never munmap'd). `prev` is the list
 * predecessor or NULL. */
static void gc_block_release(ScoopGcBlock *prev, ScoopGcBlock *block) {
    if (prev == NULL) {
        gc_blocks = block->next;
    } else {
        prev->next = block->next;
    }
    gc_table_remove((uintptr_t)block);
    gc_committed -= block->map_size;
    gc_block_count--;
    free(block->start_bits);
    gc_arena_return(block, (size_t)block->map_size);
}

/* --- containment ----------------------------------------------------- */

/* Is `p` the recorded start of an object inside a known heap block?
 * Every pointer derived from untagged memory (stack words, root slots,
 * object fields) must pass this before being dereferenced — pointers
 * into static data, the C heap, or mid-object are rejected. Interior
 * pointers are not roots: Scoop references always point at object
 * starts; raw interior pointers belong to Ptr/FFI code, which pins. */
static int is_heap_start(const void *p) {
    uintptr_t u = (uintptr_t)p;
    if ((u & (sizeof(void *) - 1)) != 0) {
        return 0;
    }
    uintptr_t base = u & ~(uintptr_t)(GC_BLOCK_SIZE - 1);
    const ScoopGcBlock *block = gc_table_lookup(base);
    if (block == NULL) {
        return 0;
    }
    if (block->kind == GC_LARGE) {
        return u == base + GC_LARGE_OBJECT_OFFSET;
    }
    if (u < base + GC_LINE_SIZE) {
        return 0; /* line 0 is the block header */
    }
    size_t word = (u - base) / sizeof(uint64_t);
    uint64_t bits = __atomic_load_n(&block->start_bits[word / 64], __ATOMIC_ACQUIRE);
    return (int)((bits >> (word % 64)) & 1);
}

bool scoop_gc_is_object_start_locked(const void *object) {
    return is_heap_start(object) != 0;
}

static void gc_record_start(ScoopGcBlock *block, const void *p) {
    size_t word = ((uintptr_t)p - (uintptr_t)block) / sizeof(uint64_t);
    (void)__atomic_fetch_or(&block->start_bits[word / 64],
                            UINT64_C(1) << (word % 64), __ATOMIC_RELEASE);
}

/* --- free-line holes ------------------------------------------------- */

/* A hole is a run of free lines, stored in-place in its own first
 * line (a line is 128B, so the header always fits). Holes from all
 * blocks share one global list — DESIGN's per-block free-line list,
 * unified and protected by the heap lock. */
typedef struct ScoopGcHole {
    struct ScoopGcHole *next;
    uint64_t lines;
} ScoopGcHole;

static ScoopGcHole *gc_holes;

static void gc_hole_push(char *start, uint64_t lines) {
    ScoopGcHole *hole = (ScoopGcHole *)start;
    hole->next = gc_holes;
    hole->lines = lines;
    gc_holes = hole;
}

/* --- mark metadata ----------------------------------------------------- */

static uint64_t gc_mark_color;
static _Atomic(uint64_t) gc_live_objects;

bool scoop_gc_mark_object_locked(const void *object) {
    ScoopObjectHeader *header = (ScoopObjectHeader *)object;
    uint64_t word = __atomic_load_n(&header->gc_word, __ATOMIC_RELAXED);
    if ((word & GC_MARK_BIT) == gc_mark_color) {
        return false;
    }
    __atomic_store_n(&header->gc_word, (word & ~GC_MARK_BIT) | gc_mark_color,
                     __ATOMIC_RELAXED);
    ScoopGcBlock *block = gc_block_of(object);
    if (block->kind == GC_SMALL) {
        size_t line =
            ((uintptr_t)object - (uintptr_t)block) / GC_LINE_SIZE;
        block->line_marks[line / 64] |= UINT64_C(1) << (line % 64);
    }
    return true;
}

void scoop_gc_heap_begin_collection_locked(void) {
    gc_mark_color ^= 1;
}

void scoop_rt_gc_init(void) {
    /* Register the compiler-emitted exact image roots before reserving the
     * heap. No managed code can run before both operations complete. */
    scoop_gc_register_image_roots(
        scoop_image_managed_globals, scoop_image_managed_global_count,
        scoop_image_immortal_objects, scoop_image_immortal_object_count);
    gc_arena_ensure();
}

/* --- sweep ---------------------------------------------------------------- */

static void gc_clear_line_bits(ScoopGcBlock *block, size_t line) {
    (void)__atomic_fetch_and(&block->start_bits[line / 4],
                             ~(UINT64_C(0xFFFF) << ((line % 4) * 16)),
                             __ATOMIC_RELAXED);
}

static void gc_sweep(void) {
    /* Rebuild the hole list from scratch: entries from the previous
     * cycle may point into blocks this sweep returns to the OS, and
     * every surviving block recomputes its free lines below anyway. */
    gc_holes = NULL;
    ScoopGcBlock *block = gc_blocks;
    ScoopGcBlock *prev = NULL;
    while (block != NULL) {
        if (block->kind == GC_LARGE) {
            const ScoopObjectHeader *object =
                (const ScoopObjectHeader *)((const char *)block + GC_LARGE_OBJECT_OFFSET);
            uint64_t word = __atomic_load_n(&object->gc_word, __ATOMIC_RELAXED);
            if ((word & GC_MARK_BIT) == gc_mark_color) {
                prev = block;
                block = block->next;
            } else {
                ScoopGcBlock *dead = block;
                block = block->next;
                gc_block_release(prev, dead);
            }
            continue;
        }
        uint64_t marks_or = 0;
        for (size_t i = 0; i < GC_LINES_PER_BLOCK / 64; i++) {
            marks_or |= block->line_marks[i];
        }
        if (marks_or == 0) {
            /* No live object: return the whole block to the arena's
             * free-block list (DESIGN 2.2 区域回收; the arena is one
             * fixed reservation, blocks are never munmap'd). */
            ScoopGcBlock *dead = block;
            block = block->next;
            gc_block_release(prev, dead);
            continue;
        }
        /* Partially live: runs of unmarked lines (skipping the header
         * line 0) become holes for reuse (DESIGN 2.1 free-line 复用). */
        char *base = (char *)block;
        size_t run_start = 0;
        for (size_t line = 1; line <= GC_LINES_PER_BLOCK; line++) {
            int marked = line < GC_LINES_PER_BLOCK
                             ? (int)((block->line_marks[line / 64] >> (line % 64)) & 1)
                             : 1; /* sentinel closing a trailing run */
            if (!marked && run_start == 0) {
                run_start = line;
            } else if (marked && run_start != 0) {
                for (size_t dead = run_start; dead < line; dead++) {
                    gc_clear_line_bits(block, dead);
                }
                gc_hole_push(base + run_start * GC_LINE_SIZE,
                             (uint64_t)(line - run_start));
                run_start = 0;
            }
        }
        /* Parity flips next cycle, so live objects keep no state here;
         * the line table is rebuilt from scratch each cycle. */
        memset(block->line_marks, 0, sizeof block->line_marks);
        prev = block;
        block = block->next;
    }
}

void scoop_gc_heap_finish_collection_locked(uint64_t live_objects) {
    gc_sweep();
    atomic_store_explicit(&gc_live_objects, live_objects, memory_order_release);
    gc_threshold = gc_committed * 2 > GC_INITIAL_THRESHOLD ? gc_committed * 2
                                                           : GC_INITIAL_THRESHOLD;
}

uint64_t scoop_rt_gc_stats(void) {
    return atomic_load_explicit(&gc_live_objects, memory_order_acquire);
}

uint64_t scoop_rt_gc_debug_block_count(void) {
    gc_lock(&gc_heap_lock, "failed to lock the heap");
    uint64_t count = gc_block_count;
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
    return count;
}

uintptr_t scoop_rt_gc_debug_arena_base(void) {
    return gc_arena_base;
}

bool scoop_rt_gc_debug_is_allocated(const void *obj) {
    gc_lock(&gc_heap_lock, "failed to lock the heap");
    bool allocated = is_heap_start(obj);
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
    return allocated;
}

/* --- allocation (runtime spec 3.1; the managed fast path is codegen's
 *     inlined bump sequence plus scoop_runtime_finish_tlab_alloc, and falls
 *     back to scoop_runtime_alloc_slow on exhaustion) ---------------------- */

static void *gc_tlab_allocate(ScoopThreadState *thread, size_t size) {
    char *p = thread->allocation.cursor;
    if (p == NULL) {
        return NULL;
    }
    /* Keep the object inside one line; skip a too-small tail. */
    char *line_end =
        (char *)(((uintptr_t)p & ~(uintptr_t)(GC_LINE_SIZE - 1)) + GC_LINE_SIZE);
    char *q = p + size <= line_end ? p : line_end;
    if (q + size > thread->allocation.limit) {
        return NULL;
    }
    thread->allocation.cursor = q + size;
    return q;
}

static bool gc_refill_tlab(ScoopThreadState *thread) {
    gc_lock(&gc_heap_lock, "failed to lock the heap");
    ScoopGcHole *hole = gc_holes;
    if (hole != NULL) {
        gc_holes = hole->next;
        thread->allocation.cursor = (char *)hole;
        thread->allocation.limit = (char *)hole + hole->lines * GC_LINE_SIZE;
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        return true;
    }
    if (gc_committed + GC_BLOCK_SIZE > gc_threshold) {
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        return false;
    }
    ScoopGcBlock *block = gc_block_new(GC_BLOCK_SIZE, GC_SMALL);
    if (block != NULL) {
        thread->allocation.cursor = (char *)block + GC_LINE_SIZE;
        thread->allocation.limit = (char *)block + GC_BLOCK_SIZE;
    }
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
    return block != NULL;
}

static void *gc_alloc_small(ScoopThreadState *thread, size_t size) {
    bool collected_for_arena = false;
    for (;;) {
        void *p = gc_tlab_allocate(thread, size);
        if (p != NULL) {
            return p;
        }
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        if (gc_refill_tlab(thread)) {
            continue;
        }
        if (collected_for_arena) {
            gc_fatal("GC arena exhausted (fixed 1 GiB in v1; growth is in the backlog)");
        }
        scoop_rt_gc_collect();
        collected_for_arena = true;
    }
}

static void *gc_alloc_large(size_t size) {
    /* One block mapping per large object (DESIGN 2.1): the header line
     * plus the object, rounded up to whole blocks. */
    size_t map_size =
        ((GC_LINE_SIZE + size) + GC_BLOCK_SIZE - 1) & ~(GC_BLOCK_SIZE - 1);
    bool collected = false;
    for (;;) {
        gc_lock(&gc_heap_lock, "failed to lock the heap");
        bool over_threshold = !collected && gc_committed + map_size > gc_threshold;
        ScoopGcBlock *block =
            over_threshold ? NULL : gc_block_new(map_size, GC_LARGE);
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        if (block != NULL) {
            return (char *)block + GC_LARGE_OBJECT_OFFSET;
        }
        if (collected) {
            gc_fatal("GC arena exhausted (fixed 1 GiB in v1; growth is in the backlog)");
        }
        scoop_rt_gc_collect();
        collected = true;
    }
}

static size_t gc_normalize_allocation_size(size_t size) {
    if (size < sizeof(ScoopObjectHeader)) {
        size = sizeof(ScoopObjectHeader);
    }
    return (size + sizeof(uint64_t) - 1) & ~(sizeof(uint64_t) - 1);
}

void scoop_runtime_finish_tlab_alloc(void *p,
                                     const ScoopTypeDescriptor *td,
                                     size_t size) {
    size = gc_normalize_allocation_size(size);
    if (p == NULL || size > GC_SMALL_MAX) {
        gc_fatal("invalid inline TLAB allocation");
    }
    memset(p, 0, size);
    ScoopObjectHeader *header = p;
    header->td = td;
    /* Mark parity = current color: unmarked when the next cycle flips it.
     * Pin remains clear because the object was zero-filled first. */
    __atomic_store_n(&header->gc_word, gc_mark_color, __ATOMIC_RELEASE);
    gc_record_start(gc_block_of(p), p);
    atomic_fetch_add_explicit(&gc_live_objects, 1, memory_order_relaxed);
}

void *scoop_runtime_alloc_slow(const ScoopTypeDescriptor *td, size_t size) {
    scoop_thread_runtime_entry();
    ScoopThreadState *thread = scoop_thread_current_required();
    size = gc_normalize_allocation_size(size);
    void *p =
        size <= GC_SMALL_MAX ? gc_alloc_small(thread, size) : gc_alloc_large(size);
    if (size <= GC_SMALL_MAX) {
        scoop_runtime_finish_tlab_alloc(p, td, size);
    } else {
        memset(p, 0, size);
        ScoopObjectHeader *header = p;
        header->td = td;
        __atomic_store_n(&header->gc_word, gc_mark_color, __ATOMIC_RELEASE);
        atomic_fetch_add_explicit(&gc_live_objects, 1, memory_order_relaxed);
    }
    return p;
}

void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size) {
    return scoop_runtime_alloc_slow(td, size);
}
