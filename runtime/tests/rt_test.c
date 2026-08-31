/* Runtime smoke test: provides scoop_main (normally emitted by the
 * compiler), exercises alloc, string concat/eq, and the String / Int /
 * Boolean print builtins, and checks that globals laid out the way
 * codegen emits string literals are readable by the runtime.
 *
 * M9: all layouts carry the 16-byte object header ({ td, gc_word },
 * fields from offset 16) and allocations come from the GC heap
 * (runtime/src/gc.c). The second half of scoop_main tests the
 * collector: allocation/collection closed loop (stats fall back),
 * TD-driven tracing (plain / array / enum scan descriptors), free-line
 * reuse, large objects, pin/handle keep-alive, and handle validation.
 *
 * Tests that need garbage to actually die run on a best-effort basis
 * against the v1 CONSERVATIVE stack scan (gc.c): they NULL their
 * locals, allocate victims inside helper functions (so the references
 * live in dead frames), and call clobber_stack() to overwrite those
 * frames before collecting. Built at -O0 this is deterministic; the
 * precise statepoint stack scan (M9 codegen task) will make it exact.
 *
 * Expected stdout is deterministic (booleans and fixed values only);
 * see the EXPECTED-OUTPUT file next to this test.
 *
 * Build & run (M8: rt.c references the C++ ABI for exceptions, hence
 * -lc++abi):
 *   cc -std=c11 -Wall -Wextra -I runtime/include runtime/src/rt.c runtime/src/gc.c runtime/tests/rt_test.c -o /tmp/scoop_rt_test -lc++abi
 *   /tmp/scoop_rt_test
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "scoop_rt.h"

/* Matches the layout of the @scoop_td_String global emitted by codegen.
 * Non-static: rt.c references it from scoop_rt_string_concat. size is
 * the fixed part (16-byte header + len); Strings have no references,
 * so the scan descriptor is NULL. */
const ScoopTypeDescriptor scoop_td_String = {1, 24, 8, NULL, NULL, NULL, NULL, 0, "String"};

/* Same layout codegen uses for StringConst globals:
 * { td, gc_word, len, data } (16-byte header, runtime spec 2.4). */
typedef struct {
    const ScoopTypeDescriptor *td;
    uint64_t gc_word;
    uint64_t len;
    char data[5];
} FiveCharConst;

static const FiveCharConst hello = {&scoop_td_String, 0, 5, {'h', 'e', 'l', 'l', 'o'}};
static const FiveCharConst world = {&scoop_td_String, 0, 5, {'w', 'o', 'r', 'l', 'd'}};

/* M6 dispatch fixtures: interface Describable; class Shape; class
 * Point : Shape, Describable (vtable = Any slots + describe). Sizes
 * include the 16-byte header. */
static int64_t point_describe(const void *self) {
    (void)self;
    return 7;
}

static const ScoopTypeDescriptor describable_td = {
    1002, 0, 8, NULL, NULL, NULL, NULL, 0, "Describable"};
static const void *const point_describable_slots[] = {(const void *)&point_describe};
static const ScoopItableEntry point_itables[] = {{&describable_td, point_describable_slots}};
static const void *const point_vtable[] = {
    (const void *)&scoop_rt_any_equals,
    (const void *)&scoop_rt_any_hashcode,
    (const void *)&scoop_rt_any_tostring,
    (const void *)&point_describe,
};
static const ScoopTypeDescriptor shape_td = {1000, 24, 8, NULL, NULL, NULL, NULL, 0, "Shape"};
static const ScoopTypeDescriptor point_td = {
    1001, 32, 8, NULL, &shape_td, point_vtable, point_itables, 1, "Point"};

/* M9 GC fixtures. */

/* class Node { value: i64, next: Node? } — plain layout, one reference
 * at object offset 24. */
typedef struct ScoopNode {
    ScoopObjectHeader header; /* 0..16 */
    int64_t value; /* 16 */
    struct ScoopNode *next; /* 24 */
} ScoopNode; /* size 32 */
static const uint64_t node_refs[] = {1, 24};
static const ScoopTypeDescriptor node_td = {
    2000, 32, 8, node_refs, NULL, NULL, NULL, 0, "Node"};

/* 64-byte plain object without references: exactly two per line, for
 * the free-line reuse test. */
typedef struct {
    ScoopObjectHeader header;
    int64_t words[6];
} ScoopBig64; /* size 64 */
static const ScoopTypeDescriptor big64_td = {2001, 64, 8, NULL, NULL, NULL, NULL, 0, "Big64"};

/* Array with reference elements (recursive SCOOP_REFS_ARRAY scan);
 * `size` in the TD is the element stride (pointer). */
static const uint64_t ref_element_scan[] = {1, 0};
static const uint64_t ref_array_scan[] = {
    SCOOP_REFS_ARRAY, 8, (uint64_t)(uintptr_t)ref_element_scan};
static const ScoopTypeDescriptor ref_array_td = {
    2100, 8, 8, ref_array_scan, NULL, NULL, NULL, 0, "Array<String>"};

/* Boxed tagged enum E { A(String), B(i64) }: { tag, payload } after
 * the header; tag at object offset 16, payload word at 24. */
typedef struct {
    ScoopObjectHeader header;
    uint64_t tag;
    uint64_t payload;
} ScoopBoxedEnum; /* size 32 */
static const uint64_t enum_a_refs[] = {1, 24}; /* variant A: one ref */
static const uint64_t enum_b_refs[] = {0}; /* variant B: no refs */
static const uint64_t enum_scan[] = {SCOOP_REFS_ENUM, 16, 2,
                                     (uint64_t)(uintptr_t)enum_a_refs,
                                     (uint64_t)(uintptr_t)enum_b_refs};
static const ScoopTypeDescriptor enum_td = {
    2002, 32, 8, enum_scan, NULL, NULL, NULL, 0, "E"};

/* Array<Nested>, where each inline element is
 * { tagged enum E, tail: String }. The sequence combines the
 * unconditional tail reference with a tag-selected payload scan;
 * the array wrapper repeats that recursive element scan by stride. */
typedef struct {
    uint64_t tag;
    uint64_t payload;
    const ScoopString *tail;
} ScoopNestedElement; /* size 24 */
static const uint64_t nested_a_scan[] = {1, 8};
static const uint64_t nested_b_scan[] = {0};
static const uint64_t nested_enum_scan[] = {
    SCOOP_REFS_ENUM, 0, 2, (uint64_t)(uintptr_t)nested_a_scan,
    (uint64_t)(uintptr_t)nested_b_scan};
static const uint64_t nested_tail_scan[] = {1, 16};
static const uint64_t nested_element_scan[] = {
    SCOOP_REFS_SEQUENCE, 2, (uint64_t)(uintptr_t)nested_tail_scan,
    (uint64_t)(uintptr_t)nested_enum_scan};
static const uint64_t nested_array_scan[] = {
    SCOOP_REFS_ARRAY, sizeof(ScoopNestedElement),
    (uint64_t)(uintptr_t)nested_element_scan};
static const ScoopTypeDescriptor nested_array_td = {
    2101, sizeof(ScoopNestedElement), 8, nested_array_scan,
    NULL, NULL, NULL, 0, "Array<Nested>"};

static ScoopNode *new_node(int64_t value, ScoopNode *next) {
    ScoopNode *node = scoop_rt_alloc(&node_td, sizeof(ScoopNode));
    node->value = value;
    node->next = next;
    return node;
}

/* Garbage is created inside helpers so the only references live in
 * the helper's (dead) frame after it returns. */
static void make_garbage_nodes(int count) {
    for (int i = 0; i < count; i++) {
        ScoopNode *junk = new_node(i, NULL);
        (void)junk;
    }
}

static void make_garbage_big64(void) {
    for (int i = 0; i < 4; i++) {
        ScoopBig64 *victim = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        victim->words[0] = -1;
    }
}

/* Enough 64B garbage to overrun one 32KB block and spill into the
 * next (600 * 64B > 255 usable lines). */
static void make_garbage_big64_many(void) {
    for (int i = 0; i < 600; i++) {
        ScoopBig64 *victim = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        victim->words[0] = -1;
    }
}

static ScoopArray *make_ref_array(void) {
    ScoopArray *array = scoop_rt_alloc(&ref_array_td, sizeof(ScoopArray) + 2 * sizeof(uint64_t));
    array->size = 2;
    const ScoopString **elements = (const ScoopString **)array->elements;
    elements[0] = scoop_rt_int_to_string(1001);
    elements[1] = scoop_rt_int_to_string(1002);
    return array;
}

static ScoopBoxedEnum *make_boxed_enum_a(void) {
    ScoopBoxedEnum *e = scoop_rt_alloc(&enum_td, sizeof(ScoopBoxedEnum));
    e->tag = 0;
    e->payload = (uint64_t)(uintptr_t)scoop_rt_int_to_string(1003);
    return e;
}

static ScoopBoxedEnum *make_boxed_enum_b(void) {
    ScoopBoxedEnum *e = scoop_rt_alloc(&enum_td, sizeof(ScoopBoxedEnum));
    e->tag = 1;
    e->payload = 0xDEADBEEF0; /* aligned non-heap word: must not be chased */
    return e;
}

static ScoopArray *make_nested_array(void) {
    ScoopArray *array = scoop_rt_alloc(
        &nested_array_td,
        sizeof(ScoopArray) + 2 * sizeof(ScoopNestedElement));
    array->size = 2;
    ScoopNestedElement *elements = (ScoopNestedElement *)array->elements;
    elements[0].tag = 0;
    elements[0].payload = (uint64_t)(uintptr_t)scoop_rt_int_to_string(1004);
    elements[0].tail = scoop_rt_int_to_string(1005);
    elements[1].tag = 1;
    elements[1].payload = UINT64_C(0xDEADBEEF0);
    elements[1].tail = scoop_rt_int_to_string(1006);
    return array;
}

/* Overwrite the dead stack region below the current frame so the
 * conservative stack scan no longer finds stale object pointers left
 * behind by helpers that have returned (v1 transition-era helper; see
 * the file header comment). */
static void clobber_stack(void) {
    volatile uint64_t buf[2048];
    for (size_t i = 0; i < 2048; i++) {
        buf[i] = 0;
    }
}

/* Static slot registered as a global root (not on the stack, so only
 * the root registration keeps its value alive). */
static ScoopNode *global_rooted;

void scoop_main(void) {
    const ScoopString *a = (const ScoopString *)&hello;
    const ScoopString *b = (const ScoopString *)&world;

    /* concat: "hello" + "world" -> "helloworld" */
    const ScoopString *concat = scoop_rt_string_concat(a, b);
    scoop_rt_println(concat);

    /* structural equality: same content, different content, length
     * mismatch */
    const ScoopString *copy = scoop_rt_string_concat(a, b);
    scoop_rt_println_boolean(scoop_rt_string_eq(concat, copy));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, b));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, concat));

    /* int / boolean output (print variants run into the println line) */
    scoop_rt_print_int(42);
    scoop_rt_println_int(-7);
    scoop_rt_print_boolean(true);
    scoop_rt_println_boolean(false);

    /* M7 primitive conversions backing core's intToString /
     * boolToString */
    scoop_rt_println(scoop_rt_int_to_string(42));
    scoop_rt_println(scoop_rt_int_to_string(-7));
    scoop_rt_println(scoop_rt_bool_to_string(true));
    scoop_rt_println(scoop_rt_bool_to_string(false));

    /* scoop_rt_array_clone (M5): independent snapshot — mutating the
     * original after the clone must not affect the copy, and the copy
     * keeps the header (td) and size. The source is a stack object
     * laid out like a codegen array (16-byte header). */
    const ScoopTypeDescriptor array_td = {
        100, 8, 8, NULL, NULL, NULL, NULL, 0, "Array<Int>"};
    struct {
        const ScoopTypeDescriptor *td;
        uint64_t gc_word;
        uint64_t size;
        int64_t data[3];
    } original = {&array_td, 0, 3, {10, 20, 30}};
    const ScoopArray *clone = scoop_rt_array_clone(&original, sizeof(int64_t), 24);
    original.data[0] = 99;
    const int64_t *snapshot = (const int64_t *)clone->elements;
    scoop_rt_println_boolean(snapshot[0] == 10 && snapshot[1] == 20 && snapshot[2] == 30);
    scoop_rt_println_boolean(clone->size == 3 && clone->header.td == &array_td);

    /* scoop_rt_box (M6): header + payload copy. */
    int64_t payload[2] = {1, 2};
    const ScoopObjectHeader *boxed = scoop_rt_box(&point_td, payload, sizeof payload);
    scoop_rt_println_boolean(boxed->td == &point_td);
    const int64_t *boxed_payload = (const int64_t *)((const char *)boxed + sizeof(ScoopObjectHeader));
    scoop_rt_println_boolean(boxed_payload[0] == 1 && boxed_payload[1] == 2);

    /* scoop_rt_is_instance (M6): own td, parent chain, itable key;
     * unrelated td does not match. */
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &point_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &shape_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &describable_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &scoop_td_String));

    /* scoop_rt_itable_lookup (M6): returns the recorded slots array. */
    const void *const *slots = scoop_rt_itable_lookup(&point_td, &describable_td);
    scoop_rt_println_boolean(slots == point_describable_slots);
    typedef int64_t (*DescribeFn)(const void *);
    scoop_rt_println_int(((DescribeFn)slots[0])(boxed));

    /* Any default methods (M6): identity equality, address hash,
     * "Object@<hex>" toString. */
    scoop_rt_println_boolean(scoop_rt_any_equals(boxed, boxed));
    scoop_rt_println_boolean(scoop_rt_any_equals(boxed, a));
    scoop_rt_println_boolean(scoop_rt_any_hashcode(boxed) == (uint64_t)(uintptr_t)boxed);
    const ScoopString *description = scoop_rt_any_tostring(boxed);
    scoop_rt_println_boolean(description->len > 7 && memcmp(description->data, "Object@", 7) == 0);

    /* scoop_rt_trap (M3) aborts the process, so it is not exercised
     * here; its trap path is covered end-to-end by EXPECT-TRAP compiler
     * fixtures. */

    /* --- M9 GC tests (runtime/src/gc.c) --- */

    /* Allocation/collection closed loop: stats grow exactly with the
     * allocation count and fall back once the garbage is dead. The
     * stack-scanned locals of scoop_main (anchor and the objects
     * above) stay alive across every collect in this function. */
    uint64_t stats_base = scoop_rt_gc_stats();
    ScoopNode *anchor = new_node(7, NULL);
    make_garbage_nodes(100);
    clobber_stack();
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_base + 101);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_stats() <= stats_base + 1);
    scoop_rt_println_boolean(anchor->value == 7);

    /* Plain-layout tracing: a node reachable only through anchor's
     * `next` field (offset 24 in node_refs) survives. */
    ScoopNode *second = new_node(8, NULL);
    anchor->next = second;
    second = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(anchor->next != NULL && anchor->next->value == 8);

    /* Free-line reuse: two rooted 64B objects share one line, four
     * victims fill the next two lines; after the collection their
     * lines are holes, and fresh allocations are served from recycled
     * lines without mapping a new block. */
    ScoopBig64 *keep1 = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
    ScoopBig64 *keep2 = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
    keep1->words[0] = 1;
    keep2->words[0] = 2;
    make_garbage_big64();
    clobber_stack();
    scoop_rt_gc_collect();
    uint64_t blocks_before_reuse = scoop_rt_gc_debug_block_count();
    ScoopBig64 *reused[8];
    for (size_t i = 0; i < 8; i++) {
        reused[i] = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        reused[i]->words[0] = (int64_t)i;
    }
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_reuse);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(keep1->words[0] == 1 && keep2->words[0] == 2);

    /* Freed-block hole regression: a block that survives a collection
     * with holes must not hand those holes out after it dies in a
     * later one (the hole list is rebuilt from scratch each cycle).
     * Overrun the current block with garbage so the rooted object and
     * its victim neighbors land in a fresh block, let that block die
     * in the next cycle, then allocate — the freed block's lines must
     * not come back. */
    make_garbage_big64_many();
    clobber_stack();
    ScoopBig64 *hold = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
    hold->words[0] = 99;
    make_garbage_big64();
    clobber_stack();
    uint64_t blocks_with_hold = scoop_rt_gc_debug_block_count();
    scoop_rt_gc_collect(); /* hold's block survives, its dead lines become holes */
    scoop_rt_println_boolean(hold->words[0] == 99);
    hold = NULL;
    clobber_stack();
    scoop_rt_gc_collect(); /* the block is fully dead: returned to the OS */
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() < blocks_with_hold);
    uint64_t stats_before_refill = scoop_rt_gc_stats();
    ScoopBig64 *refill[4];
    for (size_t i = 0; i < 4; i++) {
        refill[i] = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        refill[i]->words[0] = (int64_t)i;
    }
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_before_refill + 4);

    /* Large objects (> 64B, DESIGN 2.1): dedicated block mapping,
     * kept alive by a handle, returned to the OS once dead. */
    uint64_t blocks_before_large = scoop_rt_gc_debug_block_count();
    ScoopString *big = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + 200);
    big->len = 200;
    memset(big->data, 'x', 200);
    uint64_t big_handle = scoop_rt_get_handle(big);
    big = NULL;
    clobber_stack();
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_large + 1);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_large + 1);
    (void)scoop_rt_release_handle(big_handle);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_large);

    /* GcHandle keep-alive: the only reference to the object is the
     * handle (an integer, invisible to the stack scan). */
    ScoopNode *handled = new_node(11, NULL);
    uint64_t handle = scoop_rt_get_handle(handled);
    handled = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_gc_collect();
    const ScoopNode *back = scoop_rt_release_handle(handle);
    scoop_rt_println_boolean(back != NULL && back->value == 11);
    back = NULL;

    /* pin: survives with every reference hidden (the address is kept
     * XORed so the conservative scan cannot recognize it); after
     * unpin it is reclaimed. Heap made clean first so the stats delta
     * isolates exactly this object. */
    scoop_rt_gc_collect();
    uint64_t stats_before_pin = scoop_rt_gc_stats();
    ScoopNode *pinned = new_node(12, NULL);
    scoop_rt_pin(pinned);
    uintptr_t hidden = (uintptr_t)pinned ^ UINT64_C(0x5A5A5A5A5A5A5A5A);
    pinned = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_before_pin + 1);
    pinned = (ScoopNode *)(hidden ^ UINT64_C(0x5A5A5A5A5A5A5A5A));
    scoop_rt_unpin(pinned);
    pinned = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_before_pin);

    /* Array scan descriptor: elements referenced only from the array
     * survive (the array itself is stack-rooted). */
    ScoopArray *ref_array = make_ref_array();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopString *const *elements = (const ScoopString *const *)ref_array->elements;
    scoop_rt_println_boolean(elements[0]->len == 4 && elements[0]->data[0] == '1' &&
                             elements[1]->len == 4);

    /* Recursive array element scan: the active enum payload and both
     * unconditional tail references survive; the inactive variant's
     * aligned non-heap payload is ignored. */
    ScoopArray *nested_array = make_nested_array();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopNestedElement *nested = (const ScoopNestedElement *)nested_array->elements;
    const ScoopString *nested_payload = (const ScoopString *)(uintptr_t)nested[0].payload;
    scoop_rt_println_boolean(nested_payload->len == 4 && nested_payload->data[3] == '4' &&
                             nested[0].tail->data[3] == '5' && nested[1].tail->data[3] == '6' &&
                             nested[1].tag == 1);

    /* Enum scan descriptor: variant A keeps its payload reference;
     * variant B's payload word is not chased. */
    ScoopBoxedEnum *enum_a = make_boxed_enum_a();
    ScoopBoxedEnum *enum_b = make_boxed_enum_b();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopString *enum_payload = (const ScoopString *)(uintptr_t)enum_a->payload;
    scoop_rt_println_boolean(enum_payload->len == 4 && enum_payload->data[0] == '1');
    scoop_rt_println_boolean(enum_b->tag == 1);

    /* Global root list: a static slot registered with add_root keeps
     * its object alive without any stack reference. */
    global_rooted = new_node(13, NULL);
    scoop_rt_gc_add_root((void **)&global_rooted);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(global_rooted != NULL && global_rooted->value == 13);

    /* Scoop ABI native-root frame: the only visible object pointer is in
     * foreign heap storage, which the conservative C-stack scan cannot see.
     * The frame keeps it alive across collection and exposes the same slot a
     * future moving collector will rewrite. */
    ScoopNode *native = new_node(15, NULL);
    void **native_slot = malloc(sizeof *native_slot);
    void ***native_slots = malloc(sizeof *native_slots);
    *native_slot = native;
    native_slots[0] = native_slot;
    native = NULL;
    ScoopNativeRootFrame native_frame;
    scoop_rt_push_native_roots(&native_frame, native_slots, 1);
    scoop_rt_println_boolean(scoop_rt_gc_debug_native_root_count() == 1);
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopNode *native_back = *native_slot;
    scoop_rt_println_boolean(native_back != NULL && native_back->value == 15);
    native_back = NULL;
    scoop_rt_pop_native_roots(&native_frame);
    scoop_rt_println_boolean(scoop_rt_gc_debug_native_root_count() == 0);
    *native_slot = NULL;
    free(native_slots);
    free(native_slot);

    /* Niche no-ops: null pin/unpin/handle round-trips. */
    scoop_rt_println_boolean(scoop_rt_get_handle(NULL) == 0 && scoop_rt_release_handle(0) == NULL &&
                             scoop_rt_pin(NULL) == NULL && scoop_rt_unpin(NULL) == NULL);

    /* Arena + write-barrier card table (spec 3.6): blocks are carved
     * from one contiguous arena, and the compiler's card mark
     * `scoop_gc_card_table[addr >> 9] = 1` lands in the backing table
     * because the pointer is pre-biased by `arena_base >> 9`
     * (real[(addr - arena_base) >> 9]). Simulate the card mark the
     * compiler emits and check the slot position for both a small and
     * a large object. */
    uintptr_t arena_base = scoop_rt_gc_debug_arena_base();
    const unsigned char *card_real = scoop_gc_card_table + (arena_base >> 9);
    ScoopNode *carded = new_node(14, NULL);
    uintptr_t carded_addr = (uintptr_t)carded;
    unsigned char *carded_slot = &scoop_gc_card_table[carded_addr >> 9];
    scoop_rt_println_boolean(carded_slot == card_real + ((carded_addr - arena_base) >> 9));
    scoop_gc_card_table[carded_addr >> 9] = 1; /* the compiler's card mark */
    scoop_rt_println_boolean(card_real[(carded_addr - arena_base) >> 9] == 1);
    ScoopString *carded_big = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + 200);
    uintptr_t big_addr = (uintptr_t)carded_big;
    /* Every heap address — small-block or large-block — is inside the
     * arena (1 GiB), hence inside the card table's 2 GiB window. */
    scoop_rt_println_boolean(carded_addr >= arena_base &&
                             carded_addr < arena_base + (UINT64_C(1) << 30) &&
                             big_addr >= arena_base && big_addr < arena_base + (UINT64_C(1) << 30));
    scoop_rt_println_boolean(&scoop_gc_card_table[big_addr >> 9] ==
                             card_real + ((big_addr - arena_base) >> 9));
    carded_big = NULL;

    /* Handle validation: releasing an out-of-range handle aborts
     * (runtime spec 4.2). Checked in a child process so the test
     * binary survives; the child's "scoop gc: invalid GcHandle" goes
     * to stderr and does not pollute the expected stdout. */
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        (void)scoop_rt_release_handle(9999);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(status));
}
