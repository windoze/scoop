#include "nursery_fixture.h"

static const uint64_t element_scan[] = {1, 0};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, 8, (uintptr_t)element_scan};
static const ScoopTypeDescriptor references_td = {
    .type_id = 930,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 8,
                       .inline_stride = 8,
                       .inline_alignment = 8,
                       .inline_scan = element_scan},
    .object_scan = array_scan};
static const ScoopTypeDescriptor plain_td = {
    .type_id = 931,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 1 << 20,
                       .instance_alignment = 8}};

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &references_td, &plain_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *roots[2] = {0};
    void **slots[] = {&roots[0], &roots[1]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 2);
    roots[0] = nursery_allocate(&plain_td, 1 << 20);
    roots[1] = nursery_allocate(&references_td, 24 + 4096 * sizeof(void *));
    ((ScoopArray *)roots[1])->size = 4096;
    assert(pointer_block(roots[0])->kind == SCOOP_BLOCK_KIND_LARGE);
    assert(pointer_block(roots[1])->kind == SCOOP_BLOCK_KIND_LARGE);
    for (uint64_t round = 0; round < 4; round++) {
        unsigned char *bytes = roots[0];
        bytes[(1 << 20) - 1] = (unsigned char)(71 + round);
        scoop_rt_gc_write_barrier(bytes + sizeof(ScoopObjectHeader),
                                  (1 << 20) - sizeof(ScoopObjectHeader));
        NurseryNode **elements = (NurseryNode **)((char *)roots[1] + 24);
        elements[100] = nursery_node(round);
        elements[4000] = nursery_node(100 + round);
        scoop_rt_gc_write_barrier(&elements[100], sizeof(void *));
        scoop_rt_gc_write_barrier(&elements[4000], sizeof(void *));
        NurseryNode *previous = elements[100];
        ScoopGcMetrics before = nursery_metrics();
        nursery_collect(true);
        ScoopGcMetrics after = nursery_metrics();
        assert(after.dirty_cards - before.dirty_cards == 2);
        assert(after.old_reference_slots - before.old_reference_slots == 128);
        assert(after.traced_objects - before.traced_objects == 2);
        assert(after.promoted_bytes - before.promoted_bytes == 2 * sizeof(NurseryNode));
        assert(bytes[(1 << 20) - 1] == 71 + round);
        assert(elements[100] != previous && elements[100]->value == round);
        assert(elements[4000]->value == 100 + round);
        assert(!scoop_gc_is_young_object_locked(elements[100]));
    }
    scoop_rt_pop_native_roots(&frame);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("pointer-free large cards are skipped while reference cards retain and update young "
         "objects");
    return 0;
}
