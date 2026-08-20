#include <pthread.h>
#include <stdatomic.h>
#include <time.h>
#include <unistd.h>

#include "nursery_fixture.h"

enum { THREADS = 4, ROUNDS = 2000 };
static _Atomic(unsigned) finished;
static NurseryNode *shared;

typedef struct LargeNode {
    NurseryNode node;
    unsigned char payload[40000];
} LargeNode;

static const ScoopTypeDescriptor large_td = {
    .type_id = 802,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(LargeNode),
                       .instance_alignment = 8},
    .object_scan = nursery_node_scan,
};

static void test_counts(const ScoopTypeDescriptor *td, size_t count) {
    NurseryNode *objects[128];
    for (size_t index = 0; index < count; index++) {
        objects[index] = nursery_allocate(td, td->instance_shape.minimum_size);
        objects[index]->value = index;
        assert(scoop_rt_pin(objects[index]) == objects[index]);
        assert(scoop_rt_pin(objects[index]) == objects[index]);
    }
    nursery_collect(true);
    for (size_t index = 0; index < count; index++) {
        assert(scoop_rt_unpin(objects[index]) == objects[index]);
    }
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == count);
    /* This order repeatedly removes an interior entry and exercises the
     * moved entry's index, including after the vector has grown. */
    for (size_t turn = 0; turn < count; turn++) {
        size_t index = (turn * 37) % count;
        assert(objects[index]->value == index);
        assert(scoop_rt_unpin(objects[index]) == objects[index]);
        if (turn == count / 2 - 1) {
            nursery_collect(false);
            assert(scoop_rt_gc_stats() == count / 2);
        }
    }
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

static void *pin_worker(void *unused) {
    (void)unused;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
    scoop_thread_enter_managed((void *)boundary);
    for (unsigned round = 0; round < ROUNDS; round++) {
        assert(scoop_rt_pin(shared) == shared);
        assert(scoop_rt_pin(shared) == shared);
        assert(scoop_rt_unpin(shared) == shared);
        scoop_rt_safepoint_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
        assert(shared->value == 7331);
        assert(scoop_rt_unpin(shared) == shared);
    }
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    atomic_fetch_add_explicit(&finished, 1, memory_order_release);
    return NULL;
}

int main(void) {
    alarm(30);
#ifdef TEST_PIN_MINOR
    assert(setenv("SCOOP_GC_STRESS_MINOR", "1", 1) == 0);
#else
    assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &large_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    test_counts(&nursery_node_td, 128);
    test_counts(&large_td, 4);
    shared = nursery_node(7331);
    assert(scoop_rt_pin(shared) == shared);
    pthread_t workers[THREADS];
    for (unsigned index = 0; index < THREADS; index++) {
        assert(pthread_create(&workers[index], NULL, pin_worker, NULL) == 0);
    }
    unsigned round = 0;
    while (atomic_load_explicit(&finished, memory_order_acquire) != THREADS) {
        nursery_collect(round++ % 2 == 0);
        /* A fixed gap lets workers progress between collection requests. */
        scoop_thread_leave_managed();
        const struct timespec interval = {.tv_nsec = 100000};
        assert(nanosleep(&interval, NULL) == 0);
        scoop_thread_enter_managed((void *)boundary);
    }
    for (unsigned index = 0; index < THREADS; index++) {
        assert(pthread_join(workers[index], NULL) == 0);
    }
    nursery_collect(false);
    assert(shared->value == 7331 && scoop_rt_gc_stats() == 1);
    assert(scoop_rt_unpin(shared) == shared);
    shared = NULL;
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    ScoopGcMetrics metrics = nursery_metrics();
    assert(metrics.full_collections > 0);
#ifdef TEST_PIN_MINOR
    assert(metrics.minor_collections > 0);
#else
    assert(metrics.minor_collections == 0);
#endif
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("counted pins preserve nested, swapped, large and concurrent roots");
    return 0;
}
