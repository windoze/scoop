#include "nursery_fixture.h"
#include "../src/gc/mutator.h"

static unsigned released;

static void release_leaf(void *object) {
    assert(((NurseryNode *)object)->value == 91);
    released++;
}

static const ScoopTypeDescriptor release_td = {
    .type_id = 944,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .release_hook = release_leaf,
};

static void collect_without_movement(bool minor) {
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(minor);
    ScoopGcMetrics after = nursery_metrics();
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    assert(after.update_ns == before.update_ns);
    assert(after.minor_collections == before.minor_collections + minor);
    assert(after.full_collections == before.full_collections + !minor);
    assert(scoop_gc_heap_state.nursery_bytes == 0);
    ScoopThreadState *thread = scoop_thread_current_required();
    assert(scoop_gc_mutator_is_retired(thread->mutator));
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &release_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    collect_without_movement(false);
    NurseryNode *left = nursery_node(10), *right = nursery_node(20);
    void **slots[] = {(void **)&left, (void **)&right};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 2);
    left->next = right;
    right->next = left;
    NurseryNode *dead = nursery_node(30);
    NurseryNode *ready = nursery_allocate(&release_td, sizeof *ready);
    ready->value = 91;
    ready->header.gc_word |= GC_RELEASE_READY_BIT;
    (void)nursery_allocate(&release_td, sizeof *ready);
    collect_without_movement(false);
    assert(left->next == right && right->next == left);
    assert(!scoop_rt_gc_debug_is_allocated(dead));
    assert(!scoop_gc_is_young_object_locked(left));
    assert(scoop_rt_gc_stats() == 2 && released == 1);

    NurseryNode *young = nursery_node(40);
    young->next = young;
    left->next = young;
    scoop_rt_gc_write_barrier(&left->next, sizeof left->next);
    assert(scoop_rt_pin(young) == young);
    dead = nursery_node(41);
    collect_without_movement(true);
    assert(left->next == young && young->next == young);
    assert(!scoop_gc_is_young_object_locked(young));
    assert(!scoop_rt_gc_debug_is_allocated(dead));
    assert(scoop_rt_gc_stats() == 3 && released == 1);
    uint64_t remembered = nursery_metrics().old_reference_slots;
    collect_without_movement(true);
    assert(nursery_metrics().old_reference_slots == remembered);
    assert(scoop_rt_unpin(young) == young);

    /* A moved round followed by a no-move round must not reuse its count. */
    right->next = nursery_node(50);
    scoop_rt_gc_write_barrier(&right->next, sizeof right->next);
    uintptr_t previous = (uintptr_t)right->next;
    nursery_collect(true);
    assert(scoop_rt_gc_debug_last_moved_count() == 1);
    assert((uintptr_t)right->next != previous && right->next->value == 50);
    collect_without_movement(true);
    collect_without_movement(false);
    assert(left->value == 10 && right->value == 20 && left->next->value == 40);
    assert(scoop_rt_gc_stats() == 4 && released == 1);
    void *pinned[] = {left, right, left->next, right->next};
    for (size_t index = 0; index < 4; index++) {
        assert(scoop_rt_pin(pinned[index]) == pinned[index]);
    }
    stress_move = true;
    collect_without_movement(false);
    stress_move = false;
    for (size_t index = 0; index < 4; index++) {
        assert(scoop_rt_unpin(pinned[index]) == pinned[index]);
    }
    scoop_rt_pop_native_roots(&roots);
    collect_without_movement(false);
    assert(scoop_rt_gc_stats() == 0 && released == 1);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("zero-move full and minor preserve reclamation, promotion, pins and counters");
    return 0;
}
