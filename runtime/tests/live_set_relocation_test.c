#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"

#undef stress_move

typedef struct LargeNode {
    ScoopObjectHeader header;
    void *child;
    uint64_t value;
    unsigned char padding[40000];
} LargeNode;

static const uint64_t large_scan[] = {1, offsetof(LargeNode, child)};
static const ScoopTypeDescriptor large_td = {
    .type_id = 802,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(LargeNode),
                       .instance_alignment = 8},
    .object_scan = large_scan,
};
static const ScoopTypeDescriptor leaf_td = {
    .type_id = 803,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
};

typedef struct WideNode {
    ScoopObjectHeader header;
    void *child;
    uint64_t value;
    _Alignas(16) unsigned char padding[9120];
} WideNode;

static const uint64_t wide_scan[] = {1, offsetof(WideNode, child)};
static const ScoopTypeDescriptor wide_td = {
    .type_id = 804,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(WideNode),
                       .instance_alignment = _Alignof(WideNode)},
    .object_scan = wide_scan,
};

typedef struct ExpectedLiveSet {
    void *original[6];
    unsigned seen;
} ExpectedLiveSet;

static void check_copy_metadata(void *object, void *original) {
    size_t size = scoop_gc_object_size_locked(original);
    assert(scoop_gc_object_size_locked(object) == size);
    assert(memcmp(object, original, size) == 0);
    const ScoopTypeDescriptor *td = ((ScoopObjectHeader *)object)->td;
    assert((uintptr_t)object % td->instance_shape.instance_alignment == 0);
    ScoopGcBlockMeta *block;
    size_t word;
    assert(object_meta(object, &block, &word));
    assert(object_marked(block, word));
    if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
        size_t offset = (uintptr_t)object - (uintptr_t)block_base(block);
        size_t last = (offset + size - 1) / GC_LINE_SIZE;
        for (size_t line = offset / GC_LINE_SIZE; line <= last; line++) {
            assert(bit_test(block->line_occupied, line));
            assert(bit_test(block->line_live, line));
        }
    }
}

static void check_current_entry(void **entry, void *context) {
    ExpectedLiveSet *expected = context;
    void *object = *entry;
    const ScoopTypeDescriptor *td = ((ScoopObjectHeader *)object)->td;
    uint64_t value = td == &large_td  ? ((LargeNode *)object)->value
                     : td == &wide_td ? ((WideNode *)object)->value
                                      : ((NurseryNode *)object)->value;
    assert(value >= 1 && value <= 6);
    unsigned bit = 1u << (value - 1);
    assert((expected->seen & bit) == 0);
    expected->seen |= bit;
    void *original = expected->original[value - 1];
    assert((object == original) == (value == 3));
    ScoopGcBlockMeta *source;
    size_t word;
    assert(object_meta(original, &source, &word));
    assert(scoop_heap_forward_object(original, source, word) == object);
    check_copy_metadata(object, original);
    assert(!scoop_gc_object_was_scanned_locked(object));
    if (td->object_scan != NULL) {
        scoop_heap_claim_current_object(object);
        assert(scoop_gc_object_was_scanned_locked(object));
    }
}

static void run_case(size_t workers) {
    char count[8];
    snprintf(count, sizeof count, "%zu", workers);
    assert(setenv("SCOOP_GC_WORKERS", count, 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &large_td, &leaf_td, &wide_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 4, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    nursery_collect(false);
    assert(nursery_metrics().full_collections == 1);

    ExpectedLiveSet expected = {0};
    void **slots[6];
    for (size_t index = 0; index < 6; index++) {
        slots[index] = &expected.original[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 6);
    for (size_t index = 0; index < 3; index++) {
        expected.original[index] = nursery_node(index + 1);
    }
    NurseryNode *leaf = nursery_allocate(&leaf_td, sizeof *leaf);
    leaf->value = 4;
    expected.original[3] = leaf;
    LargeNode *large = nursery_allocate(&large_td, sizeof *large);
    large->value = 5;
    large->child = expected.original[0];
    expected.original[4] = large;
    WideNode *wide = nursery_allocate(&wide_td, sizeof *wide);
    wide->value = 6;
    wide->child = expected.original[1];
    memset(wide->padding, 0x6d, sizeof wide->padding);
    expected.original[5] = wide;
    ((NurseryNode *)expected.original[0])->next = expected.original[1];
    ((NurseryNode *)expected.original[2])->next = expected.original[1];
    assert(scoop_rt_pin(expected.original[2]) == expected.original[2]);

    /* Inspect the real phase boundary before root/field relocation consumes the set. */
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    assert(scoop_gc_stop_world());
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    scoop_gc_set_pin_frames_locked(true);
    scoop_gc_retire_allocations();
    scoop_gc_heap_state.stress_move = true;
    size_t blocks = scoop_gc_heap_begin_collection_locked(false);
    scoop_gc_mark_begin(false, blocks);
    scoop_gc_mark_roots();
    assert(scoop_gc_mark_finish(false) == 6);
    ScoopGcSurvivors survivors;
    assert(scoop_gc_heap_plan_moving_locked(false, &survivors));
    assert(moved_objects == 5 && survivors.objects == 0);
    scoop_gc_mark_visit_live(check_current_entry, &expected);
    assert(expected.seen == 63);
    /* The child intentionally ends at this boundary; full-cycle tests cover update/reclaim. */
    _exit(0);
}

int main(void) {
    const size_t workers[] = {1, 4};
    for (size_t index = 0; index < sizeof workers / sizeof *workers; index++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            run_case(workers[index]);
        }
        int status;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    puts("live entries hold copied small, large and pointer-free objects before update; pins stay");
}
