/* Scoop runtime: Immix-core garbage collector (M9, milestone9 DESIGN
 * section 2; runtime spec sections 3-4).
 *
 * v1 shape (DESIGN 5.1): single generation, non-moving, single thread,
 * no evacuation/defrag. What it provides:
 *
 * - Heap organization (DESIGN 2.1): 32KB blocks cut into 128B lines.
 *   Blocks are carved out of one contiguous address-space ARENA
 *   (gc_arena_init below) reserved at startup, so every heap address
 *   falls in a known window — the write-barrier card table is indexed
 *   against that window (see scoop_gc_card_table). Small objects
 *   (<= half a line = 64B) are bump-allocated and never straddle a
 *   line (a too-small line tail is skipped); large objects (> 64B,
 *   DESIGN's "> line 的一半") own a whole block mapping. The 64B
 *   threshold makes a dedicated 32KB block per >64B object wasteful;
 *   it is kept per DESIGN and should be revisited together with the
 *   medium-object tier (DESIGN section 6).
 * - Free-line reuse: after a collection, runs of completely free lines
 *   in partially-live blocks become holes on a global free list, which
 *   allocation drains before carving a new block. Blocks with no live
 *   object are returned to the arena's free-block list (never munmap'd:
 *   the arena is one fixed reservation).
 * - Mark-region collection (DESIGN 2.2): roots (global root list,
 *   handle table, pinned objects, thread stack) are traced precisely
 *   through TypeDescriptor scan descriptors (scoop_rt.h); mark bits
 *   live in the object header (gc_word parity bit, so sweep never
 *   touches live objects), line liveness in per-block line tables.
 * - pin / GcHandle (DESIGN 2.3, runtime spec 3.4).
 *
 * Stack roots are found by a CONSERVATIVE scan of the current thread's
 * stack (transition, see gc_scan_stack): every aligned word that
 * validates as a heap object start is a root. This is replaced by
 * precise statepoint stackmap scanning once codegen emits stackmaps
 * (milestone9 DESIGN 3.2 — codegen-side task; gc_scan_stack is the
 * interface point).
 *
 * Scanners must never chase pointers into static or foreign memory:
 * every candidate pointer — from the stack, from object fields, from
 * root slots — is dereferenced only after is_heap_start() confirms it
 * is a recorded object start inside a known heap block. Static string
 * literals and ABI exception buffers therefore never get marked or
 * reclaimed by tracing through them; out-of-heap object-like regions
 * are scanned only via scoop_rt_gc_add_root_object().
 *
 * TypeDescriptors are immortal compiler-emitted globals, so tracing a
 * stale (dead but not yet reused) object is memory-safe: its td is
 * still valid and the pointers it yields are containment-checked in
 * turn. At worst, garbage is retained one extra cycle.
 *
 * Threading: v1 assumes a single mutator thread (DESIGN 5.1); there is
 * no synchronization and no stop-the-world coordination.
 */
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#include "scoop_rt.h"

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
#define GC_PIN_BIT UINT64_C(2) /* runtime spec 3.4 pin flag */

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

/* Write-barrier card table (spec 3.6): the compiler marks
 * `scoop_gc_card_table[addr >> 9] = 1` after heap stores. This symbol
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

/* Carve `map_size` (a multiple of 32KB) out of the arena. On bump
 * exhaustion, run one last-ditch collection to recycle dead blocks
 * into the free list before giving up: the arena is fixed-size in v1,
 * so genuine exhaustion aborts (growth is in the backlog). */
static void *gc_arena_carve(size_t map_size) {
    gc_arena_ensure();
    for (int attempt = 0; attempt < 2; attempt++) {
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
        if (attempt == 0) {
            scoop_rt_gc_collect();
        }
    }
    gc_fatal("GC arena exhausted (fixed 1 GiB in v1; growth is in the backlog)");
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
    return (int)((block->start_bits[word / 64] >> (word % 64)) & 1);
}

static void gc_record_start(ScoopGcBlock *block, const void *p) {
    size_t word = ((uintptr_t)p - (uintptr_t)block) / sizeof(uint64_t);
    block->start_bits[word / 64] |= UINT64_C(1) << (word % 64);
}

/* --- free-line holes ------------------------------------------------- */

/* A hole is a run of free lines, stored in-place in its own first
 * line (a line is 128B, so the header always fits). Holes from all
 * blocks share one global list — DESIGN's per-block free-line list,
 * unified since the allocator is single-threaded. */
typedef struct ScoopGcHole {
    struct ScoopGcHole *next;
    uint64_t lines;
} ScoopGcHole;

static ScoopGcHole *gc_holes;

/* Bump area ("TLAB", DESIGN 2.1): the hole currently being consumed.
 * Allocations stay inside one line; a line tail too small for the
 * object is skipped (reclaimable only with its line, Immix-style). */
static char *gc_bump_pos;
static char *gc_bump_end;

static void gc_hole_push(char *start, uint64_t lines) {
    ScoopGcHole *hole = (ScoopGcHole *)start;
    hole->next = gc_holes;
    hole->lines = lines;
    gc_holes = hole;
}

/* --- roots ------------------------------------------------------------ */

typedef struct ScoopGcRoot {
    const void *base;
    uint32_t is_external_object;
    uint32_t reserved;
} ScoopGcRoot;

static ScoopGcRoot *gc_roots;
static size_t gc_roots_len;
static size_t gc_roots_cap;
static _Thread_local ScoopNativeRootFrame *gc_native_roots;

static void gc_root_push(const void *base, uint32_t is_external_object) {
    if (gc_roots_len == gc_roots_cap) {
        size_t new_cap = gc_roots_cap == 0 ? 16 : gc_roots_cap * 2;
        ScoopGcRoot *grown = realloc(gc_roots, new_cap * sizeof *grown);
        if (grown == NULL) {
            gc_fatal("out of memory growing the root list");
        }
        gc_roots = grown;
        gc_roots_cap = new_cap;
    }
    gc_roots[gc_roots_len].base = base;
    gc_roots[gc_roots_len].is_external_object = is_external_object;
    gc_roots[gc_roots_len].reserved = 0;
    gc_roots_len++;
}

void scoop_rt_gc_add_root(void **slot) {
    gc_root_push(slot, 0);
}

void scoop_rt_gc_add_root_object(const void *obj) {
    gc_root_push(obj, 1);
}

void scoop_rt_gc_remove_root_object(const void *obj) {
    for (size_t i = 0; i < gc_roots_len; i++) {
        if (gc_roots[i].is_external_object && gc_roots[i].base == obj) {
            gc_roots[i] = gc_roots[gc_roots_len - 1];
            gc_roots_len--;
            return;
        }
    }
    gc_fatal("attempted to remove an unknown external object root");
}

void scoop_rt_push_native_roots(ScoopNativeRootFrame *frame, void ***slots,
                                uint64_t count) {
    if (frame == NULL || (count != 0 && slots == NULL)) {
        gc_fatal("invalid native root frame");
    }
    for (ScoopNativeRootFrame *active = gc_native_roots; active != NULL;
         active = active->previous) {
        if (active == frame) {
            gc_fatal("native root frame is already active");
        }
    }
    for (uint64_t i = 0; i < count; i++) {
        if (slots[i] == NULL) {
            gc_fatal("native root frame contains a null slot address");
        }
    }
    frame->previous = gc_native_roots;
    frame->slots = slots;
    frame->count = count;
    gc_native_roots = frame;
}

void scoop_rt_pop_native_roots(ScoopNativeRootFrame *frame) {
    if (frame == NULL || gc_native_roots != frame) {
        gc_fatal("native root frames must be popped in LIFO order");
    }
    gc_native_roots = frame->previous;
    frame->previous = NULL;
    frame->slots = NULL;
    frame->count = 0;
}

/* --- handles (runtime spec 3.4 GcHandle) ------------------------------ */

/* Growable table of object pointers; the handle value is the index + 1
 * (0 is the niche). Free slots form an index-linked list, tagged in
 * bit 0 so validation can tell a live entry (8-aligned pointer) from a
 * free link. v1 never updates entries during collection (non-moving);
 * a moving collector must rewrite entries instead (DESIGN 2.3). */
static uintptr_t *gc_handles;
static size_t gc_handles_len;
static size_t gc_handles_cap;
static int64_t gc_handles_free = -1; /* first free slot index or -1 */

static uintptr_t gc_handle_encode_free(int64_t next_free) {
    return (((uint64_t)(next_free + 1)) << 1) | 1;
}

static int64_t gc_handle_decode_free(uintptr_t word) {
    return (int64_t)(word >> 1) - 1;
}

uint64_t scoop_rt_get_handle(const void *obj) {
    if (obj == NULL) {
        return 0;
    }
    /* A handle on a non-heap object (e.g. a static literal) is useless
     * but harmless: collection skips it (is_heap_start check). */
    size_t index;
    if (gc_handles_free >= 0) {
        index = (size_t)gc_handles_free;
        gc_handles_free = gc_handle_decode_free(gc_handles[index]);
        gc_handles[index] = (uintptr_t)obj;
    } else {
        if (gc_handles_len == gc_handles_cap) {
            size_t new_cap = gc_handles_cap == 0 ? 16 : gc_handles_cap * 2;
            uintptr_t *grown = realloc(gc_handles, new_cap * sizeof *grown);
            if (grown == NULL) {
                gc_fatal("out of memory growing the handle table");
            }
            gc_handles = grown;
            gc_handles_cap = new_cap;
        }
        index = gc_handles_len++;
        gc_handles[index] = (uintptr_t)obj;
    }
    return (uint64_t)index + 1;
}

const void *scoop_rt_release_handle(uint64_t handle) {
    if (handle == 0) {
        return NULL; /* niche */
    }
    if (handle > gc_handles_len || (gc_handles[handle - 1] & 1) != 0) {
        gc_fatal("invalid GcHandle"); /* runtime spec 4.2 */
    }
    const void *obj = (const void *)gc_handles[handle - 1];
    gc_handles[handle - 1] = gc_handle_encode_free(gc_handles_free);
    gc_handles_free = (int64_t)(handle - 1);
    return obj;
}

/* --- pinned objects (runtime spec 3.4 pin) ----------------------------- */

/* Registry of pinned objects so the collector can find them without
 * walking the heap: pinned objects are roots (scanned, never
 * reclaimed) even when nothing else references them. */
static const void **gc_pinned;
static size_t gc_pinned_len;
static size_t gc_pinned_cap;

const void *scoop_rt_pin(const void *obj) {
    if (obj == NULL) {
        return NULL;
    }
    if (!is_heap_start(obj)) {
        gc_fatal("scoop_rt_pin: not a GC heap object");
    }
    ScoopObjectHeader *header = (ScoopObjectHeader *)obj;
    if ((header->gc_word & GC_PIN_BIT) == 0) {
        header->gc_word |= GC_PIN_BIT;
        if (gc_pinned_len == gc_pinned_cap) {
            size_t new_cap = gc_pinned_cap == 0 ? 8 : gc_pinned_cap * 2;
            const void **grown = realloc(gc_pinned, new_cap * sizeof *grown);
            if (grown == NULL) {
                gc_fatal("out of memory growing the pinned list");
            }
            gc_pinned = grown;
            gc_pinned_cap = new_cap;
        }
        gc_pinned[gc_pinned_len++] = obj;
    }
    return obj;
}

const void *scoop_rt_unpin(const void *obj) {
    if (obj == NULL) {
        return NULL;
    }
    if (!is_heap_start(obj)) {
        gc_fatal("scoop_rt_unpin: not a GC heap object");
    }
    ScoopObjectHeader *header = (ScoopObjectHeader *)obj;
    if ((header->gc_word & GC_PIN_BIT) == 0) {
        gc_fatal("scoop_rt_unpin: object is not pinned");
    }
    header->gc_word &= ~GC_PIN_BIT;
    for (size_t i = 0; i < gc_pinned_len; i++) {
        if (gc_pinned[i] == obj) {
            gc_pinned[i] = gc_pinned[--gc_pinned_len];
            break;
        }
    }
    return obj;
}

/* --- marking ------------------------------------------------------------ */

static uint64_t gc_mark_color; /* parity value meaning "marked this cycle" */
static const void **gc_work;
static size_t gc_work_len;
static size_t gc_work_cap;
static uint64_t gc_marked_count; /* objects marked in the current cycle */
static uint64_t gc_live_objects; /* scoop_rt_gc_stats */

static void gc_work_push(const void *obj) {
    if (gc_work_len == gc_work_cap) {
        size_t new_cap = gc_work_cap == 0 ? 256 : gc_work_cap * 2;
        const void **grown = realloc(gc_work, new_cap * sizeof *grown);
        if (grown == NULL) {
            gc_fatal("out of memory growing the mark worklist");
        }
        gc_work = grown;
        gc_work_cap = new_cap;
    }
    gc_work[gc_work_len++] = obj;
}

/* Mark a validated heap object and queue it for tracing. */
static void gc_mark(const void *obj) {
    ScoopObjectHeader *header = (ScoopObjectHeader *)obj;
    if ((header->gc_word & GC_MARK_BIT) == gc_mark_color) {
        return; /* already marked this cycle */
    }
    header->gc_word = (header->gc_word & ~GC_MARK_BIT) | gc_mark_color;
    ScoopGcBlock *block = gc_block_of(obj);
    if (block->kind == GC_SMALL) {
        /* Small objects never straddle lines: one bit covers it. */
        size_t line = ((uintptr_t)obj - (uintptr_t)block) / GC_LINE_SIZE;
        block->line_marks[line / 64] |= UINT64_C(1) << (line % 64);
    }
    /* Large blocks: sweep reads liveness off the object header. */
    gc_marked_count++;
    gc_work_push(obj);
}

static void gc_trace_slot(const void *const *slot) {
    const void *target = *slot;
    if (is_heap_start(target)) {
        gc_mark(target);
    }
}

/* Scan one inline value as directed by the recursive descriptor in
 * scoop_rt.h. Plain offsets and enum tag offsets are relative to base. */
static void gc_trace_descriptor(const void *base, const uint64_t *table) {
    if (table == NULL) {
        return;
    }
    if (table[0] == SCOOP_REFS_ARRAY) {
        uint64_t stride = table[1];
        const uint64_t *element_scan = (const uint64_t *)(uintptr_t)table[2];
        uint64_t count = *(const uint64_t *)((const char *)base + 16);
        const char *elements = (const char *)base + 24;
        for (uint64_t i = 0; i < count; i++) {
            gc_trace_descriptor(elements + i * stride, element_scan);
        }
        return;
    }
    if (table[0] == SCOOP_REFS_ENUM) {
        uint64_t tag_offset = table[1];
        uint64_t variant_count = table[2];
        uint64_t tag = *(const uint64_t *)((const char *)base + tag_offset);
        if (tag >= variant_count) {
            gc_fatal("enum value tag out of range");
        }
        gc_trace_descriptor(base, (const uint64_t *)(uintptr_t)table[3 + tag]);
        return;
    }
    if (table[0] == SCOOP_REFS_SEQUENCE) {
        uint64_t child_count = table[1];
        for (uint64_t i = 0; i < child_count; i++) {
            gc_trace_descriptor(base, (const uint64_t *)(uintptr_t)table[2 + i]);
        }
        return;
    }
    uint64_t count = table[0];
    for (uint64_t i = 0; i < count; i++) {
        gc_trace_slot((const void *const *)((const char *)base + table[1 + i]));
    }
}

static void gc_trace_object(const void *obj) {
    const uint64_t *refs = ((const ScoopObjectHeader *)obj)->td->ref_offsets;
    gc_trace_descriptor(obj, refs);
}

/* --- conservative stack scan (v1 transition) ---------------------------- */

/* Highest address of the mutator stack, recorded by scoop_rt_gc_init
 * from main's frame. NULL until then; collections skip the scan. */
static const char *gc_stack_base;

void scoop_rt_gc_init(void *stack_base) {
    gc_stack_base = stack_base;
    /* Reserve the heap arena and set up the card table at startup. */
    gc_arena_ensure();
}

static void gc_scan_range(const char *lo, const char *hi) {
    uintptr_t p = ((uintptr_t)lo + sizeof(void *) - 1) & ~(uintptr_t)(sizeof(void *) - 1);
    for (; p + sizeof(void *) <= (uintptr_t)hi; p += sizeof(void *)) {
        /* memcpy: the stack is untyped storage; read words without
         * tripping strict aliasing. */
        const void *word;
        memcpy(&word, (const void *)p, sizeof word);
        if (is_heap_start(word)) {
            gc_mark(word);
        }
    }
}

/* TRANSITION (v1): conservative stack roots. Every aligned word between
 * the current frame and the recorded stack base that validates as a
 * heap object start is treated as a root. Memory-safe because
 * is_heap_start gates every dereference; imprecise because stale words
 * keep garbage alive (one cycle of over-retention at worst — the
 * mutator overwrites its frames as it runs). Non-moving by luck: v1
 * never moves objects, so conservative roots need no updating. This is
 * replaced by precise statepoint stackmap scanning (milestone9 DESIGN
 * 3.2) once codegen emits __llvm_stackmaps; the rest of the collector
 * only ever calls this function for stack roots. */
static void gc_scan_stack(void) {
    if (gc_stack_base == NULL) {
        return;
    }
    /* Spill callee-saved registers into this frame so roots living only
     * in registers land in the scanned range. */
    jmp_buf registers;
    (void)setjmp(registers);
    gc_scan_range((const char *)&registers, gc_stack_base);
}

/* --- sweep ---------------------------------------------------------------- */

static void gc_clear_line_bits(ScoopGcBlock *block, size_t line) {
    block->start_bits[line / 4] &= ~(UINT64_C(0xFFFF) << ((line % 4) * 16));
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
            if ((object->gc_word & GC_MARK_BIT) == gc_mark_color) {
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

/* --- collection entry points ---------------------------------------------- */

void scoop_rt_gc_collect(void) {
    /* New cycle: with the parity flipped, every object starts unmarked
     * (allocations set the bit to the *previous* color). */
    gc_mark_color ^= 1;
    /* Retire the bump area; its remaining lines are unmarked and become
     * holes at sweep like any other free lines. */
    gc_bump_pos = NULL;
    gc_bump_end = NULL;
    gc_marked_count = 0;

    /* Roots (runtime spec 3.3). */
    for (size_t i = 0; i < gc_roots_len; i++) {
        if (gc_roots[i].is_external_object) {
            /* Object-like region outside the heap (ABI exception
             * buffer): trace its references, never mark or reclaim it. */
            gc_trace_object(gc_roots[i].base);
        } else {
            gc_trace_slot((const void *const *)gc_roots[i].base);
        }
    }
    for (ScoopNativeRootFrame *frame = gc_native_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t i = 0; i < frame->count; i++) {
            gc_trace_slot((const void *const *)frame->slots[i]);
        }
    }
    for (size_t i = 0; i < gc_handles_len; i++) {
        if ((gc_handles[i] & 1) == 0) {
            const void *obj = (const void *)gc_handles[i];
            if (is_heap_start(obj)) {
                gc_mark(obj);
            }
        }
    }
    for (size_t i = 0; i < gc_pinned_len; i++) {
        gc_mark(gc_pinned[i]);
    }
    gc_scan_stack();

    /* Trace. */
    while (gc_work_len > 0) {
        gc_trace_object(gc_work[--gc_work_len]);
    }

    gc_sweep();
    gc_live_objects = gc_marked_count;
    gc_threshold = gc_committed * 2 > GC_INITIAL_THRESHOLD ? gc_committed * 2
                                                           : GC_INITIAL_THRESHOLD;
}

uint64_t scoop_rt_gc_stats(void) {
    return gc_live_objects;
}

uint64_t scoop_rt_gc_debug_block_count(void) {
    return gc_block_count;
}

uintptr_t scoop_rt_gc_debug_arena_base(void) {
    return gc_arena_base;
}

uint64_t scoop_rt_gc_debug_root_count(void) {
    return (uint64_t)gc_roots_len;
}

uint64_t scoop_rt_gc_debug_native_root_count(void) {
    uint64_t count = 0;
    for (ScoopNativeRootFrame *frame = gc_native_roots; frame != NULL;
         frame = frame->previous) {
        count += frame->count;
    }
    return count;
}

/* --- allocation (runtime spec 3.1 slow path; the managed fast path is
 *     codegen's inlined bump sequence — M9 codegen task — and lands in
 *     the same slow path) --------------------------------------------------- */

static void *gc_alloc_small(size_t size) {
    for (;;) {
        char *p = gc_bump_pos;
        if (p != NULL) {
            /* Keep the object inside one line; skip a too-small tail. */
            char *line_end =
                (char *)(((uintptr_t)p & ~(uintptr_t)(GC_LINE_SIZE - 1)) + GC_LINE_SIZE);
            char *q = p + size <= line_end ? p : line_end;
            if (q + size <= gc_bump_end) {
                gc_bump_pos = q + size;
                gc_record_start(gc_block_of(q), q);
                return q;
            }
        }
        ScoopGcHole *hole = gc_holes;
        if (hole != NULL) {
            gc_holes = hole->next;
            gc_bump_pos = (char *)hole;
            gc_bump_end = (char *)hole + hole->lines * GC_LINE_SIZE;
            continue;
        }
        if (gc_committed + GC_BLOCK_SIZE > gc_threshold) {
            /* Threshold reached: collect, then retry the hole list.
             * The post-cycle threshold (2x committed) guarantees the
             * retry either finds a hole or falls through to a fresh
             * block — no thrash loop. */
            scoop_rt_gc_collect();
            continue;
        }
        ScoopGcBlock *block = gc_block_new(GC_BLOCK_SIZE, GC_SMALL);
        gc_bump_pos = (char *)block + GC_LINE_SIZE; /* line 0: header */
        gc_bump_end = (char *)block + GC_BLOCK_SIZE;
    }
}

static void *gc_alloc_large(size_t size) {
    /* One block mapping per large object (DESIGN 2.1): the header line
     * plus the object, rounded up to whole blocks. */
    size_t map_size =
        ((GC_LINE_SIZE + size) + GC_BLOCK_SIZE - 1) & ~(GC_BLOCK_SIZE - 1);
    if (gc_committed + map_size > gc_threshold) {
        scoop_rt_gc_collect();
    }
    ScoopGcBlock *block = gc_block_new(map_size, GC_LARGE);
    return (char *)block + GC_LARGE_OBJECT_OFFSET;
}

void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size) {
    if (size < sizeof(ScoopObjectHeader)) {
        size = sizeof(ScoopObjectHeader); /* defensive: callers include the header */
    }
    size = (size + sizeof(uint64_t) - 1) & ~(sizeof(uint64_t) - 1);
    void *p = size <= GC_SMALL_MAX ? gc_alloc_small(size) : gc_alloc_large(size);
    ScoopObjectHeader *header = p;
    header->td = td;
    /* Mark parity = current color: unmarked when the next cycle flips
     * it. Pin clear. */
    header->gc_word = gc_mark_color;
    gc_live_objects++;
    return p;
}

/* ---- M9 compiler-side contracts (docs/milestone9/DESIGN.md 3.1) ---- */

/* scoop_gc_card_table (write-barrier card table, spec 3.6) is defined
 * in the arena section above: a pointer variable pre-biased by
 * arena_base >> 9. */

/* Safepoint poll: the compiler emits `call void @scoop_rt_safepoint()`
 * at function entries and loop back edges (spec 14.2). v1 is
 * single-threaded and collects synchronously, so the poll is a no-op;
 * it becomes the thread handshake point for stop-the-world or
 * incremental collection later. */
void scoop_rt_safepoint(void) {
    /* v1: no-op by design */
}
