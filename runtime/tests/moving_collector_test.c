#include <assert.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread.h"

typedef struct TestLeaf {
    ScoopObjectHeader header;
    uint64_t value;
} TestLeaf;

typedef struct TestNode {
    ScoopObjectHeader header;
    void *child;
    uint64_t value;
} TestNode;

typedef struct TestPair {
    ScoopObjectHeader header;
    void *left;
    void *right;
    uint64_t value;
} TestPair;

typedef struct TestLarge {
    ScoopObjectHeader header;
    uint64_t value;
    unsigned char payload[5000];
} TestLarge;

static const uint64_t one_root_scan[] = {1, 0};
static const uint64_t node_scan[] = {1, offsetof(TestNode, child)};
static const uint64_t pair_scan[] = {
    2,
    offsetof(TestPair, left),
    offsetof(TestPair, right),
};

static const ScoopTypeDescriptor leaf_td = {
    .type_id = 1,
    .size = sizeof(TestLeaf),
    .align = _Alignof(TestLeaf),
    .ref_offsets = NULL,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .name = "TestLeaf",
};

static const ScoopTypeDescriptor node_td = {
    .type_id = 2,
    .size = sizeof(TestNode),
    .align = _Alignof(TestNode),
    .ref_offsets = node_scan,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .name = "TestNode",
};

static const ScoopTypeDescriptor pair_td = {
    .type_id = 3,
    .size = sizeof(TestPair),
    .align = _Alignof(TestPair),
    .ref_offsets = pair_scan,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .name = "TestPair",
};

static const ScoopTypeDescriptor large_td = {
    .type_id = 4,
    .size = sizeof(TestLarge),
    .align = _Alignof(TestLarge),
    .ref_offsets = NULL,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .name = "TestLarge",
};

static const TestLeaf immortal_leaf = {
    .header = {.td = &leaf_td, .gc_word = 0},
    .value = UINT64_C(0xfeedface),
};

static const TestLeaf unregistered_immortal = {
    .header = {.td = &leaf_td, .gc_word = 0},
    .value = UINT64_C(0xbad1dea),
};

static TestLeaf unregistered_external = {
    .header = {.td = &leaf_td, .gc_word = 0},
    .value = UINT64_C(0xdecafbad),
};

static void *image_global;
static void *explicit_root;

static const ScoopManagedGlobalDescriptor managed_globals[] = {
    {.writable_base = &image_global, .scan = one_root_scan},
};

static const ScoopImmortalObjectDescriptor immortal_objects[] = {
    {
        .object_start = &immortal_leaf,
        .object_size = sizeof(immortal_leaf),
        .td = &leaf_td,
    },
};

static TestLeaf *new_leaf(uint64_t value) {
    TestLeaf *leaf = scoop_gc_alloc_internal(&leaf_td, sizeof *leaf);
    leaf->value = value;
    return leaf;
}

static TestNode *new_node(void *child, uint64_t value) {
    TestNode *node = scoop_gc_alloc_internal(&node_td, sizeof *node);
    node->child = child;
    node->value = value;
    return node;
}

static TestPair *new_pair(void *left, void *right, uint64_t value) {
    TestPair *pair = scoop_gc_alloc_internal(&pair_td, sizeof *pair);
    pair->left = left;
    pair->right = right;
    pair->value = value;
    return pair;
}

static TestLarge *new_large(uint64_t value) {
    TestLarge *large = scoop_gc_alloc_internal(&large_td, sizeof *large);
    large->value = value;
    large->payload[0] = 0x12;
    large->payload[sizeof large->payload - 1] = 0x34;
    return large;
}

static void *collect_with_stack_root(void *root) {
    _Alignas(16) uintptr_t frame[4] = {0};
    memcpy(&frame[0], &root, sizeof root);
    frame[2] =
        (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)&frame[0],
                             (uintptr_t)&frame[2]);
    memcpy(&root, &frame[0], sizeof root);
    return root;
}

static void collect_without_stack_root(void) {
    (void)collect_with_stack_root(NULL);
}

typedef void (*AbortProbe)(const void *argument);

static void expect_abort(AbortProbe probe, const void *argument) {
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        (void)close(STDERR_FILENO);
        probe(argument);
        _exit(0);
    }

    int status = 0;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status));
    assert(WTERMSIG(status) == SIGABRT);
}

static void collect_invalid_root(const void *root) {
    (void)collect_with_stack_root((void *)root);
}

static void register_null_root(const void *unused) {
    (void)unused;
    scoop_rt_gc_add_root(NULL);
}

static void test_stack_root_and_invalid_addresses(void) {
    TestLeaf *old = new_leaf(10);
    TestLeaf *current = collect_with_stack_root(old);
    assert(current != old);
    assert(current->value == 10);
    assert(scoop_rt_gc_debug_allocation_size(current) == sizeof *current);
    assert(scoop_rt_gc_debug_is_allocated(current));
    assert(!scoop_rt_gc_debug_is_allocated(old));
    assert(scoop_rt_gc_debug_last_moved_count() == 1);

    expect_abort(collect_invalid_root, old);
    expect_abort(collect_invalid_root, (const char *)current + 8);
    expect_abort(collect_invalid_root, (const void *)(uintptr_t)0x1230);
    expect_abort(collect_invalid_root, &unregistered_external);
    expect_abort(collect_invalid_root, &unregistered_immortal);
    expect_abort(register_null_root, NULL);
}

static void test_cycles_and_forwarding_identity(void) {
    TestPair *cycle = new_pair(NULL, NULL, 20);
    cycle->left = cycle;
    cycle->right = cycle;
    TestPair *moved_cycle = collect_with_stack_root(cycle);
    assert(moved_cycle != cycle);
    assert(moved_cycle->left == moved_cycle);
    assert(moved_cycle->right == moved_cycle);
    assert(moved_cycle->value == 20);
    assert(scoop_rt_gc_debug_last_moved_count() == 1);

    TestLeaf *shared = new_leaf(21);
    TestPair *owner = new_pair(shared, shared, 22);
    TestPair *moved_owner = collect_with_stack_root(owner);
    assert(moved_owner != owner);
    assert(moved_owner->left == moved_owner->right);
    assert(moved_owner->left != shared);
    assert(((TestLeaf *)moved_owner->left)->value == 21);
    assert(moved_owner->value == 22);
    assert(scoop_rt_gc_debug_last_moved_count() == 2);
}

static void test_global_and_explicit_roots(void) {
    TestLeaf *child = new_leaf(30);
    TestNode *owner = new_node(child, 31);
    image_global = owner;
    collect_without_stack_root();
    TestNode *moved_owner = image_global;
    assert(moved_owner != owner);
    assert(moved_owner->child != child);
    assert(((TestLeaf *)moved_owner->child)->value == 30);
    assert(moved_owner->value == 31);
    image_global = NULL;

    explicit_root = new_leaf(32);
    TestLeaf *old = explicit_root;
    collect_without_stack_root();
    assert(explicit_root != old);
    assert(((TestLeaf *)explicit_root)->value == 32);
    explicit_root = NULL;
}

static void test_frame_root_families(void) {
    void *native_slot = new_leaf(40);
    void *native_old = native_slot;
    void **native_slots[] = {&native_slot};
    ScoopNativeRootFrame native_frame = {0};
    scoop_rt_push_native_roots(&native_frame, native_slots, 1);
    collect_without_stack_root();
    scoop_rt_pop_native_roots(&native_frame);
    assert(native_slot != native_old);
    assert(((TestLeaf *)native_slot)->value == 40);

    struct {
        void *ref;
    } native_region = {.ref = new_leaf(41)};
    void *native_region_old = native_region.ref;
    ScoopNativeRegionRootEntry native_region_entry = {
        .base = &native_region,
        .scan = one_root_scan,
    };
    ScoopNativeRegionRootFrame native_region_frame = {0};
    scoop_rt_push_native_region_roots(&native_region_frame,
                                      &native_region_entry, 1);
    collect_without_stack_root();
    scoop_rt_pop_native_region_roots(&native_region_frame);
    assert(native_region.ref != native_region_old);
    assert(((TestLeaf *)native_region.ref)->value == 41);

    struct {
        void *ref;
    } caller_region = {.ref = new_leaf(42)};
    void *caller_old = caller_region.ref;
    ScoopCallerRootEntry caller_entry = {
        .base = &caller_region,
        .scan = one_root_scan,
    };
    ScoopCallerRootFrame caller_frame = {0};
    scoop_rt_push_caller_roots(&caller_frame, &caller_entry, 1);
    collect_without_stack_root();
    scoop_rt_pop_caller_roots(&caller_frame);
    assert(caller_region.ref != caller_old);
    assert(((TestLeaf *)caller_region.ref)->value == 42);

    struct {
        void *ref;
    } compiler_region = {.ref = new_leaf(43)};
    void *compiler_old = compiler_region.ref;
    ScoopCallerRootEntry compiler_entry = {
        .base = &compiler_region,
        .scan = one_root_scan,
    };
    ScoopCompilerRootFrame compiler_frame = {0};
    scoop_rt_push_compiler_roots(&compiler_frame, &compiler_entry, 1);
    collect_without_stack_root();
    scoop_rt_pop_compiler_roots(&compiler_frame);
    assert(compiler_region.ref != compiler_old);
    assert(((TestLeaf *)compiler_region.ref)->value == 43);
}

static void test_handle_external_and_immortal_roots(void) {
    TestLeaf *handled = new_leaf(50);
    uint64_t handle = scoop_rt_get_handle(handled);
    collect_without_stack_root();
    TestLeaf *moved_handled = (TestLeaf *)scoop_rt_resolve_handle(handle);
    assert(moved_handled != handled);
    assert(moved_handled->value == 50);
    assert(scoop_rt_release_handle(handle) == moved_handled);

    TestNode external = {
        .header = {.td = &node_td, .gc_word = 0},
        .child = new_leaf(51),
        .value = 52,
    };
    void *external_child = external.child;
    scoop_rt_gc_add_root_object(&external);
    collect_without_stack_root();
    assert(external.child != external_child);
    assert(((TestLeaf *)external.child)->value == 51);
    assert(external.value == 52);
    scoop_rt_gc_remove_root_object(&external);

    assert(collect_with_stack_root((void *)&immortal_leaf) ==
           (void *)&immortal_leaf);
    assert(immortal_leaf.value == UINT64_C(0xfeedface));
}

static void test_pin_partial_block_and_unpin(void) {
    TestLeaf *child = new_leaf(60);
    TestNode *pinned = new_node(child, 61);
    TestLeaf *sibling = new_leaf(62);
    assert(scoop_rt_pin(pinned) == pinned);

    TestLeaf *moved_sibling = collect_with_stack_root(sibling);
    assert(moved_sibling != sibling);
    assert(moved_sibling->value == 62);
    assert(scoop_rt_gc_debug_is_allocated(pinned));
    assert(pinned->child != child);
    assert(((TestLeaf *)pinned->child)->value == 60);

    pinned->child = NULL;
    assert(scoop_rt_unpin(pinned) == pinned);
    TestNode *moved_pinned = collect_with_stack_root(pinned);
    assert(moved_pinned != pinned);
    assert(moved_pinned->child == NULL);
    assert(moved_pinned->value == 61);
}

static void test_large_exact_size_and_no_conservative_retention(void) {
    TestLarge *large = new_large(70);
    TestLarge *moved = collect_with_stack_root(large);
    assert(moved != large);
    assert(moved->value == 70);
    assert(moved->payload[0] == 0x12);
    assert(moved->payload[sizeof moved->payload - 1] == 0x34);
    assert(scoop_rt_gc_debug_allocation_size(moved) == sizeof *moved);

    TestLeaf *garbage = new_leaf(71);
    uintptr_t heap_looking_integer = (uintptr_t)garbage;
    collect_without_stack_root();
    assert(!scoop_rt_gc_debug_is_allocated(
        (const void *)heap_looking_integer));
}

int main(void) {
    uintptr_t managed_boundary_marker = 0;
    scoop_thread_runtime_init();
    scoop_gc_stackmaps_init();
    scoop_gc_register_image_roots(
        managed_globals,
        sizeof managed_globals / sizeof managed_globals[0],
        immortal_objects,
        sizeof immortal_objects / sizeof immortal_objects[0]);
    scoop_gc_heap_init();
    scoop_thread_attach_main(&managed_boundary_marker);
    scoop_rt_gc_add_root(&explicit_root);

    test_stack_root_and_invalid_addresses();
    test_cycles_and_forwarding_identity();
    test_global_and_explicit_roots();
    test_frame_root_families();
    test_handle_external_and_immortal_roots();
    test_pin_partial_block_and_unpin();
    test_large_exact_size_and_no_conservative_retention();

    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("moving collector fake-platform tests passed");
    return 0;
}
