#include "nursery_fixture.h"

#undef calloc
extern void *calloc(size_t count, size_t size);
static bool fail_copy_group;

void *scoop_test_slot_calloc(size_t count, size_t size) {
    if (fail_copy_group && count == 1 && size == 6 * sizeof(uintptr_t)) {
        fail_copy_group = false;
        return NULL;
    }
    return calloc(count, size);
}

static const uint64_t reference_scan[] = {1, 16};
#define SLOT_TYPE(ID, SIZE, SCAN)                                                          \
    {.type_id = ID,                                                                       \
     .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,              \
                        .minimum_size = SIZE, .instance_alignment = 8},                    \
     .object_scan = SCAN}
static const ScoopTypeDescriptor types[] = {
    SLOT_TYPE(951, 16, NULL), SLOT_TYPE(952, 24, reference_scan),
    SLOT_TYPE(953, 32, reference_scan), SLOT_TYPE(954, 32, reference_scan),
    SLOT_TYPE(955, 40, reference_scan), SLOT_TYPE(956, 64, reference_scan),
    SLOT_TYPE(957, 128, reference_scan), SLOT_TYPE(958, 256, reference_scan),
};
#undef SLOT_TYPE

#ifdef SCOOP_TEST_SLOT_PROMOTION_PARALLEL
static void check_failed_reservation(void) {
    void *root = NULL;
    void **slot = &root;
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, &slot, 1);
    NurseryNode *first = NULL;
    for (size_t index = 0; index < 16384; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = root;
        root = node;
        if (index == 0) {
            first = node;
        }
    }
    void *original = root;
    uint64_t fallbacks = nursery_metrics().promotion_fallbacks;
    uint64_t copied = scoop_gc_heap_state.copied_bytes;
    fail_copy_group = true;
    nursery_collect(true);
    assert(!fail_copy_group && nursery_metrics().promotion_fallbacks == fallbacks + 1);
    assert(root == original && scoop_gc_heap_state.copied_bytes == copied);
    assert(pointer_block(first)->slot_size == sizeof *first);
    NurseryNode *node = root;
    for (size_t count = 16384; count != 0; count--) {
        assert(node != NULL && node->value == count - 1);
        node = node->next;
    }
    assert(node == NULL);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}
#endif

static void check_layout(size_t type, size_t prefix, bool mixed, bool pinned, bool survivor) {
    size_t size = types[type].instance_shape.minimum_size;
    size_t count = GC_REGULAR_MAX * 3 / 4 / size;
    void **objects = calloc(count, sizeof *objects);
    void ***slots = malloc(count * sizeof *slots);
    uintptr_t *original = malloc(count * sizeof *original);
    assert(objects != NULL && slots != NULL && original != NULL);
    for (size_t index = 0; index < count; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, count);
    if (prefix != 0) {
        const ScoopTypeDescriptor *td = prefix == 24 ? &types[1] : &types[5];
        nursery_allocate(td, prefix);
    }
    for (size_t index = 0; index < count; index++) {
        const ScoopTypeDescriptor *td = &types[type];
        if (mixed && index % 8 == 7) {
            td = &types[4];
        } else if (type == 2 && index % 2 != 0) {
            td = &types[3];
        }
        size_t bytes = td->instance_shape.minimum_size;
        objects[index] = td == &types[2] ? nursery_node(0) : nursery_allocate(td, bytes);
        memset((char *)objects[index] + sizeof(ScoopObjectHeader), 0x6d,
               bytes - sizeof(ScoopObjectHeader));
        if (bytes > sizeof(ScoopObjectHeader)) {
            *(void **)((char *)objects[index] + 16) = index == 0 ? NULL : objects[0];
        }
        original[index] = (uintptr_t)objects[index];
        if (index % 7 == 6) {
            objects[index] = NULL;
        }
    }
    ScoopGcBlockMeta *block = pointer_block(objects[0]);
    assert(pointer_block((void *)original[count - 1]) == block);
    if (pinned) {
        assert(scoop_rt_pin(objects[0]) == objects[0]);
    }
    uint64_t promoted = nursery_metrics().promoted_bytes;
    nursery_collect(true);
    if (survivor) {
        assert(objects[0] == (void *)original[0]);
        assert(block->generation == SCOOP_GC_YOUNG && block->slot_size == 0);
        assert(nursery_metrics().promoted_bytes == promoted);
        nursery_collect(true);
    }
    assert(nursery_metrics().promoted_bytes > promoted);
    bool eligible = !mixed && prefix != 24 && !pinned;
    for (size_t index = 0; index < count; index++) {
        if (objects[index] == NULL) {
            continue;
        }
        void *object = objects[index];
        const ScoopTypeDescriptor *td = ((ScoopObjectHeader *)object)->td;
        assert((uintptr_t)object % td->instance_shape.instance_alignment == 0);
        assert(((uintptr_t)object == original[index]) == (eligible || pinned));
        assert(pointer_block(object)->generation == SCOOP_GC_OLD);
        assert(pointer_block(object)->slot_size == (pinned ? 0 : td->instance_shape.minimum_size));
        if (size > sizeof(ScoopObjectHeader)) {
            assert(*(void **)((char *)object + 16) == (index == 0 ? NULL : objects[0]));
        }
        if (size > 24) {
            assert(((unsigned char *)object)[size - 1] == 0x6d);
        }
    }
    if (eligible) {
        /* Full sweep must retain the newly classified grid, including dead mixed
         * prefix bytes, and make its holes available to future promotions. */
        nursery_collect(false);
        assert(objects[0] == (void *)original[0] && block->slot_size == size);
    }
    if (pinned) {
        assert(scoop_rt_unpin(objects[0]) == objects[0]);
    }
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    free(original);
    free(slots);
    free(objects);
}

int main(void) {
    assert(setenv("SCOOP_GC_OLD_SLOTS", "2", 1) == 0);
#ifdef SCOOP_TEST_SLOT_PROMOTION_PARALLEL
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
#else
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
#endif
    const ScoopTypeDescriptor *registered[sizeof types / sizeof types[0] + 1];
    registered[sizeof types / sizeof types[0]] = &nursery_node_td;
    for (size_t index = 0; index < sizeof types / sizeof types[0]; index++) {
        registered[index] = &types[index];
    }
    scoop_thread_runtime_init();
    scoop_test_image_init(registered, sizeof registered / sizeof registered[0], NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    for (size_t type = 0; type < sizeof types / sizeof types[0]; type++) {
        check_layout(type, 0, false, false, false);
    }
    check_layout(2, 64, false, false, false);
    check_layout(2, 24, false, false, false);
    check_layout(2, 0, true, false, false);
    check_layout(2, 0, false, true, false);
#ifdef SCOOP_TEST_SLOT_PROMOTION_PARALLEL
    check_failed_reservation();
#endif
    scoop_gc_heap_state.survivor_enabled = true;
    check_layout(2, 0, false, false, true);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("slot promotion preserves exact grids, mixed types, aliases, pins and survivor age");
}
