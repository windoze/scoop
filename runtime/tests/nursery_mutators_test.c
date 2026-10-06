#include <pthread.h>

#include "nursery_fixture.h"

typedef struct SharedCard {
    ScoopObjectHeader header;
    NurseryNode *slots[2];
} SharedCard;

static const uint64_t shared_scan[] = {2, offsetof(SharedCard, slots[0]),
                                       offsetof(SharedCard, slots[1])};
static const ScoopTypeDescriptor shared_td = {
    .type_id = 805,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(SharedCard),
                       .instance_alignment = 8},
    .object_scan = shared_scan,
};
static SharedCard *shared;

static void *mutator(void *argument) {
    size_t slot = (size_t)(uintptr_t)argument;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = 0;
    scoop_thread_enter_managed(&boundary);
    NurseryNode *node = NULL;
    void **root = (void **)&node;
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, &root, 1);
    for (uint64_t index = 0; index < 2000; index++) {
        node = nursery_node(slot * 10000 + index);
        shared->slots[slot] = node;
        scoop_rt_gc_write_barrier(&shared->slots[slot], sizeof(void *));
    }
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

int main(void) {
    assert(setenv("SCOOP_GC_STRESS_MINOR", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &shared_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    shared = nursery_allocate(&shared_td, sizeof *shared);
    assert(scoop_rt_pin(shared) == shared);
    nursery_collect(true);
    assert(!scoop_gc_is_young_object_locked(shared));
    scoop_thread_leave_managed();

    pthread_t threads[2];
    for (size_t index = 0; index < 2; index++) {
        assert(pthread_create(&threads[index], NULL, mutator, (void *)(uintptr_t)index) == 0);
    }
    for (size_t index = 0; index < 2; index++) {
        assert(pthread_join(threads[index], NULL) == 0);
    }

    scoop_thread_enter_managed(&boundary);
    nursery_collect(true);
    assert(shared->slots[0]->value == 1999);
    assert(shared->slots[1]->value == 11999);
    assert(nursery_metrics().minor_collections > 2);
    assert(scoop_rt_unpin(shared) == shared);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("concurrent nursery mutators preserve writes to the same old card");
    return 0;
}
