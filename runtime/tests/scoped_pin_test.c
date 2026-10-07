#include <pthread.h>
#include <stdatomic.h>
#include <time.h>
#include <unistd.h>

#include "nursery_fixture.h"

static void nested_frames(void) {
    NurseryNode *object = nursery_node(91);
    ScoopPinFrame outer, inner;
    scoop_rt_push_pin_frame(&outer, object);
    object->next = nursery_node(73);
    scoop_rt_gc_write_barrier(&object->next, sizeof object->next);
    scoop_rt_push_pin_frame(&inner, object);
    assert(scoop_rt_pin(object) == object);
    nursery_collect(true);
    assert(outer.object == object && inner.object == object);
    assert(object->value == 91 && object->next->value == 73);
    assert(scoop_rt_unpin(object) == object);
    nursery_collect(false);
    assert(outer.object == object && inner.object == object);
    scoop_rt_pop_pin_frame(&inner);
    nursery_collect(false);
    assert(outer.object == object && object->next->value == 73);
    assert(scoop_rt_gc_stats() == 2);

    /* A handle keeps the object alive after the last frame exits, while full
     * stress collection verifies that the address can move again. */
    uint64_t handle = scoop_rt_get_handle(object);
    scoop_rt_pop_pin_frame(&outer);
    assert(scoop_thread_current_required()->pin_frames == NULL);
    nursery_collect(false);
    NurseryNode *moved = (NurseryNode *)scoop_rt_resolve_handle(handle);
#ifndef TEST_PIN_MINOR
    assert(moved != object);
#endif
    assert(moved->value == 91 && moved->next->value == 73);
    assert(scoop_rt_release_handle(handle) == moved);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

static void explicit_pin_outlives_frame(void) {
    NurseryNode *object = nursery_node(27);
    ScoopPinFrame frame;
    scoop_rt_push_pin_frame(&frame, object);
    assert(scoop_rt_pin(object) == object);
    nursery_collect(true);
    scoop_rt_pop_pin_frame(&frame);
    nursery_collect(false);
    assert(object->value == 27 && scoop_rt_gc_stats() == 1);
    assert(scoop_rt_unpin(object) == object);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

static NurseryNode *shared;
static _Atomic(unsigned) finished;

static void *borrow_worker(void *unused) {
    (void)unused;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    _Alignas(16) uintptr_t anchor[4] = {0, 0, boundary, 0};
    scoop_thread_enter_managed((void *)boundary);
    for (unsigned round = 0; round < 1000; round++) {
        ScoopPinFrame outer, inner;
        scoop_rt_push_pin_frame(&outer, shared);
        scoop_rt_safepoint_impl(0x1010, (uintptr_t)anchor, (uintptr_t)&anchor[2]);
        scoop_rt_push_pin_frame(&inner, shared);
        scoop_rt_safepoint_impl(0x1010, (uintptr_t)anchor, (uintptr_t)&anchor[2]);
        assert(shared->value == 91);
        scoop_rt_pop_pin_frame(&inner);
        scoop_rt_pop_pin_frame(&outer);
    }
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    atomic_fetch_add_explicit(&finished, 1, memory_order_release);
    return NULL;
}

static void concurrent_frames(void) {
    shared = nursery_node(91);
    ScoopPinFrame frame;
    scoop_rt_push_pin_frame(&frame, shared);
    pthread_t threads[4];
    for (unsigned index = 0; index < 4; index++) {
        assert(pthread_create(&threads[index], NULL, borrow_worker, NULL) == 0);
    }
    unsigned round = 0;
    while (atomic_load_explicit(&finished, memory_order_acquire) != 4) {
        nursery_collect(round++ % 2 == 0);
        scoop_thread_leave_managed();
        const struct timespec interval = {.tv_nsec = 100000};
        assert(nanosleep(&interval, NULL) == 0);
        scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    }
    for (unsigned index = 0; index < 4; index++) {
        assert(pthread_join(threads[index], NULL) == 0);
    }
    assert(shared->value == 91);
    scoop_rt_pop_pin_frame(&frame);
    shared = NULL;
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

int main(void) {
    alarm(30);
#ifdef TEST_PIN_MINOR
    assert(setenv("SCOOP_GC_STRESS_MINOR", "1", 1) == 0);
#else
    assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    nested_frames();
    explicit_pin_outlives_frame();
    concurrent_frames();
    ScoopGcMetrics metrics = nursery_metrics();
    assert(metrics.full_collections > 0);
#ifdef TEST_PIN_MINOR
    assert(metrics.minor_collections > 0);
#endif
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("scoped pins preserve roots and compose with explicit pins");
    return 0;
}
