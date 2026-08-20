#include "nursery_fixture.h"

static const uint64_t element_scan[] = {1, 0};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, 8, (uintptr_t)element_scan};
static const ScoopTypeDescriptor array_td = {
    .type_id = 802,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 8,
                       .inline_stride = 8,
                       .inline_alignment = 8,
                       .inline_scan = element_scan},
    .object_scan = array_scan,
};
static unsigned released;
static void release_object(void *object) {
    assert(((NurseryNode *)object)->value == 99);
    released++;
}
static const ScoopTypeDescriptor release_td = {
    .type_id = 803,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .release_hook = release_object,
};

static void check_old_cycle(void **roots) {
    roots[0] = nursery_node(10);
    void *first = roots[0];
    assert(scoop_gc_is_young_object_locked(first));
    nursery_collect(true);
    assert(roots[0] != first && !scoop_gc_is_young_object_locked(roots[0]));
    NurseryNode *left = nursery_node(20), *right = nursery_node(30);
    left->next = right;
    right->next = left;
    ((NurseryNode *)roots[0])->next = left;
    scoop_rt_gc_write_barrier(&((NurseryNode *)roots[0])->next, sizeof(void *));
    void *garbage = nursery_node(40);
    nursery_collect(true);
    assert(!scoop_rt_gc_debug_is_allocated(garbage));
    NurseryNode *current = ((NurseryNode *)roots[0])->next;
    assert(current != left && current->value == 20);
    assert(current->next->value == 30 && current->next->next == current);
    assert(!scoop_gc_is_young_object_locked(current));
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.minor_collections == before.minor_collections + 1);
    assert(after.old_reference_slots == before.old_reference_slots);
    assert(after.traced_objects == before.traced_objects);
}

static void check_array_cards(void **roots) {
    size_t count = 4096;
    ScoopArray *array = nursery_allocate(&array_td, 24 + count * sizeof(void *));
    roots[1] = array;
    array->size = count;
    assert(!scoop_gc_is_young_object_locked(array));
    NurseryNode **elements = (NurseryNode **)((char *)array + 24);
    elements[100] = nursery_node(100);
    elements[4000] = nursery_node(4000);
    scoop_rt_gc_write_barrier(&elements[100], sizeof(void *));
    scoop_rt_gc_write_barrier(&elements[4000], sizeof(void *));
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(elements[100]->value == 100 && elements[4000]->value == 4000);
    assert(!scoop_gc_is_young_object_locked(elements[100]));
    assert(after.dirty_cards == before.dirty_cards + 2);
    assert(after.old_reference_slots - before.old_reference_slots == 128);
    assert(after.traced_objects - before.traced_objects == 2);
}

static void check_pin_and_handle(void **roots) {
    roots[2] = nursery_node(50);
    roots[3] = nursery_node(60);
    ((NurseryNode *)roots[2])->next = roots[3];
    void *pinned = roots[2], *sibling = roots[3];
    assert(scoop_rt_pin(pinned) == pinned);
    uint64_t handle = scoop_rt_get_handle(sibling);
    nursery_collect(true);
    assert(roots[2] == pinned && roots[3] == sibling);
    assert(!scoop_gc_is_young_object_locked(pinned));
    assert(scoop_rt_resolve_handle(handle) == sibling);
    assert(((NurseryNode *)pinned)->next == sibling);
    assert(scoop_rt_unpin(pinned) == pinned);
    assert(scoop_rt_release_handle(handle) == sibling);
    nursery_collect(false);
    assert(((NurseryNode *)roots[2])->next == roots[3]);
    assert(((NurseryNode *)roots[3])->value == 60);
}

static void check_pretenuring(void) {
    NurseryNode *ready = nursery_allocate(&release_td, sizeof *ready);
    ready->value = 99;
    ready->header.gc_word |= GC_RELEASE_READY_BIT;
    NurseryNode *unready = nursery_allocate(&release_td, sizeof *unready);
    assert(!scoop_gc_is_young_object_locked(ready));
    assert(!scoop_gc_is_young_object_locked(unready));
    nursery_collect(true);
    assert(released == 0 && scoop_rt_gc_debug_is_allocated(ready));
    nursery_collect(false);
    assert(released == 1);
}

static void check_recursive_native_region(void) {
    struct Region {
        NurseryNode *first;
        uint64_t padding[64];
        NurseryNode *last;
    } region = {.first = nursery_node(81), .last = nursery_node(82)};
    const uint64_t scan[] = {2, offsetof(struct Region, first), offsetof(struct Region, last)};
    ScoopNativeRegionRootEntry entry = {.base = &region, .scan = scan};
    ScoopNativeRegionRootFrame frame;
    scoop_rt_push_native_region_roots(&frame, &entry, 1);
    NurseryNode *first = region.first, *last = region.last;
    nursery_collect(true);
    assert(region.first != first && region.last != last);
    assert(region.first->value == 81 && region.last->value == 82);
    scoop_rt_pop_native_region_roots(&frame);
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &array_td, &release_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *roots[4] = {0};
    void **slots[] = {&roots[0], &roots[1], &roots[2], &roots[3]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 4);
    check_old_cycle(roots);
    check_array_cards(roots);
    check_pin_and_handle(roots);
    check_pretenuring();
    check_recursive_native_region();
    scoop_rt_pop_native_roots(&frame);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    assert(nursery_metrics().promoted_bytes > 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("nursery graphs, dirty array ranges, pin, handle and release passed");
    return 0;
}
