#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <unistd.h>

#include "../src/thread/testing.h"
#include "nursery_fixture.h"

static _Atomic(unsigned) requested, completed;
static _Atomic(bool) finish;
static _Thread_local bool rival;
static bool hold_refill;
static unsigned stolen;

static void await(_Atomic(unsigned) *value, unsigned expected) {
    while (atomic_load_explicit(value, memory_order_acquire) < expected) {
        sched_yield();
    }
}

static void take_nursery(unsigned round) {
    atomic_store_explicit(&requested, round, memory_order_release);
    await(&completed, round);
}

void scoop_thread_test_point(ScoopThreadTestPoint point) {
    if (point != SCOOP_TEST_NURSERY_RETRY || rival || !hold_refill || stolen == 3) {
        return;
    }
    take_nursery(++stolen + 1);
}

static void *claim_capacity(void *unused) {
    (void)unused;
    rival = true;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    unsigned round = 0;
    while (!atomic_load_explicit(&finish, memory_order_acquire)) {
        unsigned request = atomic_load_explicit(&requested, memory_order_acquire);
        if (request == round) {
            sched_yield();
            continue;
        }
        scoop_thread_enter_managed((void *)boundary);
        assert(nursery_node(request)->value == request);
        scoop_thread_leave_managed();
        round = request;
        atomic_store_explicit(&completed, round, memory_order_release);
    }
    scoop_rt_detach_foreign_thread();
    return NULL;
}

int main(void) {
    alarm(15);
    assert(setenv("SCOOP_GC_STRESS_MINOR", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    pthread_t thread;
    assert(pthread_create(&thread, NULL, claim_capacity, NULL) == 0);
    take_nursery(1);
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    hold_refill = true;
    NurseryNode *node = nursery_node(7331);
    hold_refill = false;
    assert(node->value == 7331 && stolen == 3);
    ScoopGcMetrics metrics = nursery_metrics();
    assert(metrics.minor_collections == 4 && metrics.full_collections == 0);
    scoop_thread_leave_managed();
    atomic_store_explicit(&finish, true, memory_order_release);
    assert(pthread_join(thread, NULL) == 0);
    scoop_thread_enter_managed((void *)boundary);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("nursery contention retries without false arena exhaustion");
    return 0;
}
