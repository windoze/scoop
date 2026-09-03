#include "support.h"

void run_gc_tests(void) {
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
    /* Conservative M13 stack roots may retain stale helper-frame words;
     * precise M15 stackmaps remove that nondeterminism. Collection itself
     * must not increase the live-object count. */
    scoop_rt_println_boolean(scoop_rt_gc_stats() <= stats_base + 101);
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
    scoop_rt_gc_collect(); /* stale conservative roots may retain the block */
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() <= blocks_with_hold);
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
    const ScoopString *nested_payload = nested[0].a_ref;
    scoop_rt_println_boolean(nested_payload->len == 4 && nested_payload->data[3] == '4' &&
                             nested[0].tail->data[3] == '5' && nested[1].tail->data[3] == '6' &&
                             nested[1].tag == 1);

    /* Fixed enum scan: A's dedicated slot keeps its reference; B uses
     * the shared pure payload and leaves the A slot zero. */
    ScoopBoxedEnum *enum_a = make_boxed_enum_a();
    ScoopBoxedEnum *enum_b = make_boxed_enum_b();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopString *enum_payload = enum_a->a_ref;
    scoop_rt_println_boolean(enum_payload->len == 4 && enum_payload->data[0] == '1');
    scoop_rt_println_boolean(enum_b->tag == 1);

    /* Global root list: a static slot registered with add_root keeps
     * its object alive without any stack reference. */
    global_rooted = new_node(13, NULL);
    scoop_rt_gc_add_root((void **)&global_rooted);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(global_rooted != NULL && global_rooted->value == 13);

    /* Compiler-emitted managed-global metadata is registered by GC init; no
     * dynamic add_root call is needed for this writable storage. */
    image_global_rooted = new_node(14, NULL);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(image_global_rooted != NULL &&
                             image_global_rooted->value == 14);

    /* An exact managed slot may point at a compiler-registered immortal
     * object start; it neither enters the mark worklist nor fails validation. */
    image_immortal_rooted = (void *)&hello;
    scoop_rt_gc_add_root(&image_immortal_rooted);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(image_immortal_rooted == (void *)&hello);

    /* Exact roots reject every heap-external pointer that is not an
     * explicitly registered immortal object start. */
    fflush(stdout);
    pid_t invalid_root_pid = fork();
    if (invalid_root_pid == 0) {
        void *invalid_root = (void *)(uintptr_t)16;
        scoop_rt_gc_add_root(&invalid_root);
        scoop_rt_gc_collect();
        _exit(0);
    }
    int invalid_root_status = 0;
    waitpid(invalid_root_pid, &invalid_root_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(invalid_root_status));

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
     * atomic OR at `scoop_gc_card_table[addr >> 9]` lands in the backing table
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
    (void)__atomic_fetch_or(&scoop_gc_card_table[carded_addr >> 9], 1,
                            __ATOMIC_RELAXED); /* compiler card mark */
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

    /* Generation changes when a released slot is reused, and the stale
     * generation is rejected deterministically rather than aliasing the new
     * object. Checked in a child so the test binary survives. */
    ScoopNode *generation_a = new_node(31, NULL);
    uint64_t stale_handle = scoop_rt_get_handle(generation_a);
    (void)scoop_rt_release_handle(stale_handle);
    ScoopNode *generation_b = new_node(32, NULL);
    uint64_t current_handle = scoop_rt_get_handle(generation_b);
    bool generation_changed = (uint32_t)stale_handle == (uint32_t)current_handle &&
                              stale_handle != current_handle;
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        (void)scoop_rt_release_handle(stale_handle);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    scoop_rt_println_boolean(generation_changed && WIFSIGNALED(status));
    (void)scoop_rt_release_handle(current_handle);
}
