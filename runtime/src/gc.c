/* Scoop runtime: Immix-core garbage collector (M9, milestone9 DESIGN
 * section 2; runtime spec sections 3-4).
 *
 * M13 shape: single generation, non-moving, multiple mutators with one
 * stop-the-world collector, no evacuation/defrag. What it provides:
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
 * M13 registers every OS thread in a TLS ScoopThreadState, moves
 * stack/native-root/TLAB ownership there, and coordinates collection with a
 * cooperative epoch handshake. Mutators allocate from disjoint owner-only
 * TLABs; heap and root registries use separate locks with heap-before-roots
 * ordering.
 */
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#include "scoop_rt.h"
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
static pthread_mutex_t gc_roots_lock = PTHREAD_MUTEX_INITIALIZER;

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

/* --- roots ------------------------------------------------------------ */

typedef struct ScoopGcRoot {
    const void *base;
    uint32_t is_external_object;
    uint32_t reserved;
} ScoopGcRoot;

static ScoopGcRoot *gc_roots;
static size_t gc_roots_len;
static size_t gc_roots_cap;

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
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    gc_root_push(slot, 0);
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
}

void scoop_rt_gc_add_root_object(const void *obj) {
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    gc_root_push(obj, 1);
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
}

void scoop_rt_gc_remove_root_object(const void *obj) {
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    for (size_t i = 0; i < gc_roots_len; i++) {
        if (gc_roots[i].is_external_object && gc_roots[i].base == obj) {
            gc_roots[i] = gc_roots[gc_roots_len - 1];
            gc_roots_len--;
            gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
            return;
        }
    }
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    gc_fatal("attempted to remove an unknown external object root");
}

void scoop_rt_push_native_roots(ScoopNativeRootFrame *frame, void ***slots,
                                uint64_t count) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        gc_fatal("native roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || (count != 0 && slots == NULL)) {
        gc_fatal("invalid native root frame");
    }
    for (ScoopNativeRootFrame *active = thread->native_roots; active != NULL;
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
    frame->previous = thread->native_roots;
    frame->slots = slots;
    frame->count = count;
    thread->native_roots = frame;
}

void scoop_rt_pop_native_roots(ScoopNativeRootFrame *frame) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        gc_fatal("native roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || thread->native_roots != frame) {
        gc_fatal("native root frames must be popped in LIFO order");
    }
    thread->native_roots = frame->previous;
    frame->previous = NULL;
    frame->slots = NULL;
    frame->count = 0;
}

/* --- handles (runtime spec 3.4 GcHandle) ------------------------------ */

typedef struct ScoopGcHandleSlot {
    const void *object;
    int64_t next_free;
    uint32_t generation;
    bool live;
    bool retired;
} ScoopGcHandleSlot;

/* The low 32 bits encode slot+1 and the high 32 bits encode generation.
 * Generation 0 and handle 0 are invalid. A generation-exhausted slot is
 * permanently retired instead of permitting ABA. */
static ScoopGcHandleSlot *gc_handles;
static size_t gc_handles_len;
static size_t gc_handles_cap;
static int64_t gc_handles_free = -1; /* first free slot index or -1 */

static uint64_t gc_handle_encode(size_t index, uint32_t generation) {
    return ((uint64_t)generation << 32) | ((uint64_t)index + 1);
}

static ScoopGcHandleSlot *gc_handle_resolve_locked(uint64_t handle) {
    uint32_t encoded_slot = (uint32_t)handle;
    uint32_t generation = (uint32_t)(handle >> 32);
    if (encoded_slot == 0 || generation == 0) {
        return NULL;
    }
    size_t index = (size_t)encoded_slot - 1;
    if (index >= gc_handles_len) {
        return NULL;
    }
    ScoopGcHandleSlot *slot = &gc_handles[index];
    if (!slot->live || slot->generation != generation) {
        return NULL;
    }
    return slot;
}

uint64_t scoop_rt_get_handle(const void *obj) {
    if (obj == NULL) {
        return 0;
    }
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    size_t index;
    if (gc_handles_free >= 0) {
        index = (size_t)gc_handles_free;
        ScoopGcHandleSlot *slot = &gc_handles[index];
        gc_handles_free = slot->next_free;
        slot->generation++;
        if (slot->generation == 0 || slot->retired) {
            gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
            gc_fatal("GcHandle generation exhausted");
        }
        slot->object = obj;
        slot->next_free = -1;
        slot->live = true;
    } else {
        if (gc_handles_len == gc_handles_cap) {
            size_t new_cap = gc_handles_cap == 0 ? 16 : gc_handles_cap * 2;
            ScoopGcHandleSlot *grown =
                realloc(gc_handles, new_cap * sizeof *grown);
            if (grown == NULL) {
                gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
                gc_fatal("out of memory growing the handle table");
            }
            gc_handles = grown;
            gc_handles_cap = new_cap;
        }
        index = gc_handles_len++;
        gc_handles[index] = (ScoopGcHandleSlot){
            .object = obj,
            .next_free = -1,
            .generation = 1,
            .live = true,
            .retired = false,
        };
    }
    uint64_t handle = gc_handle_encode(index, gc_handles[index].generation);
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    return handle;
}

const void *scoop_rt_release_handle(uint64_t handle) {
    if (handle == 0) {
        return NULL; /* niche */
    }
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    ScoopGcHandleSlot *slot = gc_handle_resolve_locked(handle);
    if (slot == NULL) {
        gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
        gc_fatal("invalid or stale GcHandle");
    }
    const void *obj = slot->object;
    slot->object = NULL;
    slot->live = false;
    size_t index = (size_t)((uint32_t)handle - 1);
    if (slot->generation == UINT32_MAX) {
        slot->retired = true;
        slot->next_free = -1;
    } else {
        slot->next_free = gc_handles_free;
        gc_handles_free = (int64_t)index;
    }
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    return obj;
}

const void *scoop_rt_resolve_handle(uint64_t handle) {
    if (handle == 0) {
        return NULL;
    }
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    ScoopGcHandleSlot *slot = gc_handle_resolve_locked(handle);
    if (slot == NULL) {
        gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
        gc_fatal("invalid or stale GcHandle");
    }
    const void *object = slot->object;
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    return object;
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
    gc_lock(&gc_heap_lock, "failed to lock the heap");
    if (!is_heap_start(obj)) {
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        gc_fatal("scoop_rt_pin: not a GC heap object");
    }
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    ScoopObjectHeader *header = (ScoopObjectHeader *)obj;
    uint64_t old_word = __atomic_fetch_or(&header->gc_word, GC_PIN_BIT,
                                          __ATOMIC_ACQ_REL);
    if ((old_word & GC_PIN_BIT) == 0) {
        if (gc_pinned_len == gc_pinned_cap) {
            size_t new_cap = gc_pinned_cap == 0 ? 8 : gc_pinned_cap * 2;
            const void **grown = realloc(gc_pinned, new_cap * sizeof *grown);
            if (grown == NULL) {
                gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
                gc_unlock(&gc_heap_lock, "failed to unlock the heap");
                gc_fatal("out of memory growing the pinned list");
            }
            gc_pinned = grown;
            gc_pinned_cap = new_cap;
        }
        gc_pinned[gc_pinned_len++] = obj;
    }
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
    return obj;
}

const void *scoop_rt_unpin(const void *obj) {
    if (obj == NULL) {
        return NULL;
    }
    gc_lock(&gc_heap_lock, "failed to lock the heap");
    if (!is_heap_start(obj)) {
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        gc_fatal("scoop_rt_unpin: not a GC heap object");
    }
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    ScoopObjectHeader *header = (ScoopObjectHeader *)obj;
    uint64_t old_word = __atomic_fetch_and(&header->gc_word, ~GC_PIN_BIT,
                                           __ATOMIC_ACQ_REL);
    if ((old_word & GC_PIN_BIT) == 0) {
        gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        gc_fatal("scoop_rt_unpin: object is not pinned");
    }
    bool found = false;
    for (size_t i = 0; i < gc_pinned_len; i++) {
        if (gc_pinned[i] == obj) {
            gc_pinned[i] = gc_pinned[--gc_pinned_len];
            found = true;
            break;
        }
    }
    if (!found) {
        gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
        gc_unlock(&gc_heap_lock, "failed to unlock the heap");
        gc_fatal("pinned registry is inconsistent");
    }
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
    return obj;
}

/* --- marking ------------------------------------------------------------ */

static uint64_t gc_mark_color; /* parity value meaning "marked this cycle" */
static const void **gc_work;
static size_t gc_work_len;
static size_t gc_work_cap;
static uint64_t gc_marked_count; /* objects marked in the current cycle */
static _Atomic(uint64_t) gc_live_objects; /* scoop_rt_gc_stats */

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
    uint64_t word = __atomic_load_n(&header->gc_word, __ATOMIC_RELAXED);
    if ((word & GC_MARK_BIT) == gc_mark_color) {
        return; /* already marked this cycle */
    }
    __atomic_store_n(&header->gc_word, (word & ~GC_MARK_BIT) | gc_mark_color,
                     __ATOMIC_RELAXED);
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
 * scoop_rt.h. Every plain reference offset is relative to base. */
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

void scoop_rt_gc_init(void) {
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

static void gc_scan_native_roots(const ScoopThreadState *thread) {
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t i = 0; i < frame->count; i++) {
            gc_trace_slot((const void *const *)frame->slots[i]);
        }
    }
}

static void gc_scan_caller_roots(const ScoopThreadState *thread) {
    for (ScoopCallerRootFrame *frame = thread->caller_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t i = 0; i < frame->count; i++) {
            gc_trace_descriptor(frame->entries[i].base, frame->entries[i].scan);
        }
    }
}

static void gc_scan_frozen_managed_segments(const ScoopThreadState *thread) {
    for (ScoopThreadTransition *transition = thread->current_transition;
         transition != NULL; transition = transition->previous) {
        uintptr_t low = transition->managed_stack_low;
        uintptr_t high = transition->managed_stack_high;
        if (low < (uintptr_t)thread->stack_low || low >= high ||
            high > (uintptr_t)thread->stack_high) {
            gc_fatal("native transition contains an invalid managed stack segment");
        }
        gc_scan_range((const char *)low, (const char *)high);
    }
}

/* M13 transition: each managed mutator publishes a stable SP and a setjmp
 * register spill before publishing parked/collector. The STW collector may
 * then conservatively scan those stable regions. Native-safe threads keep
 * running, so their active native stacks are deliberately skipped while the
 * LIFO transition chain contributes only frozen outer managed segments. M15
 * replaces these conservative ranges with precise stackmap locations. */
static void gc_scan_thread(const ScoopThreadState *thread) {
    ScoopThreadMode mode = atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_PARKED || mode == SCOOP_THREAD_COLLECTOR) {
        if (thread->parked_from == SCOOP_THREAD_MANAGED) {
            uintptr_t stack_low = (uintptr_t)thread->stack_low;
            uintptr_t stack_high = (uintptr_t)thread->stack_high;
            uintptr_t parked_sp = (uintptr_t)thread->parked_sp;
            uintptr_t managed_boundary = (uintptr_t)thread->managed_stack_boundary;
            if (parked_sp == 0 || parked_sp < stack_low || managed_boundary == 0 ||
                managed_boundary > stack_high || parked_sp >= managed_boundary) {
                gc_fatal("parked thread published an invalid stack pointer");
            }
            gc_scan_range((const char *)&thread->register_spill,
                          (const char *)&thread->register_spill +
                              sizeof thread->register_spill);
            gc_scan_range(thread->parked_sp, thread->managed_stack_boundary);
        } else if (thread->parked_from != SCOOP_THREAD_NATIVE_BORROWED) {
            gc_fatal("parked thread has an invalid source mode");
        }
    } else if (mode != SCOOP_THREAD_NATIVE_SAFE) {
        gc_fatal("collector observed a non-quiescent thread");
    }
    gc_scan_frozen_managed_segments(thread);
    gc_scan_caller_roots(thread);
    gc_scan_native_roots(thread);
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

/* --- collection entry points ---------------------------------------------- */

void scoop_rt_gc_collect(void) {
    if (!scoop_thread_begin_collection()) {
        return;
    }
    /* The world is stopped before metadata locks are acquired. Native-safe
     * threads may still run root APIs, so holding roots through sweep gives
     * the collection one coherent root/pin snapshot. */
    gc_lock(&gc_heap_lock, "failed to lock the heap");
    gc_lock(&gc_roots_lock, "failed to lock the root registry");

    for (ScoopThreadState *thread = scoop_thread_collection_registry_head();
         thread != NULL; thread = thread->registry_next) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
    }
    /* New cycle: with the parity flipped, every object starts unmarked
     * (allocations set the bit to the *previous* color). */
    gc_mark_color ^= 1;
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
    for (size_t i = 0; i < gc_handles_len; i++) {
        if (gc_handles[i].live) {
            const void *obj = gc_handles[i].object;
            if (is_heap_start(obj)) {
                gc_mark(obj);
            }
        }
    }
    for (size_t i = 0; i < gc_pinned_len; i++) {
        gc_mark(gc_pinned[i]);
    }
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head();
         thread != NULL; thread = thread->registry_next) {
        gc_scan_thread(thread);
    }

    /* Trace. */
    while (gc_work_len > 0) {
        gc_trace_object(gc_work[--gc_work_len]);
    }

    gc_sweep();
    atomic_store_explicit(&gc_live_objects, gc_marked_count, memory_order_release);
    gc_threshold = gc_committed * 2 > GC_INITIAL_THRESHOLD ? gc_committed * 2
                                                           : GC_INITIAL_THRESHOLD;
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    gc_unlock(&gc_heap_lock, "failed to unlock the heap");
    scoop_thread_end_collection();
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

uint64_t scoop_rt_gc_debug_root_count(void) {
    gc_lock(&gc_roots_lock, "failed to lock the root registry");
    uint64_t count = (uint64_t)gc_roots_len;
    gc_unlock(&gc_roots_lock, "failed to unlock the root registry");
    return count;
}

uint64_t scoop_rt_gc_debug_native_root_count(void) {
    ScoopThreadState *thread = scoop_thread_current();
    if (thread == NULL) {
        return 0;
    }
    uint64_t count = 0;
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        count += frame->count;
    }
    return count;
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

/* ---- M9 compiler-side contracts (docs/milestone9/DESIGN.md 3.1) ---- */

/* scoop_gc_card_table (write-barrier card table, spec 3.6) is defined
 * in the arena section above: a pointer variable pre-biased by
 * arena_base >> 9. */

/* Safepoint poll: the fast path compares the current thread's observed
 * epoch with the global epoch. The slow path spills registers, publishes
 * its SP, parks, and waits for the single STW collector. */
void scoop_rt_safepoint(void) {
    scoop_thread_poll();
}
