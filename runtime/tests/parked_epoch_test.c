#include <assert.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread/internal.h"
#include "platform/image_fixture.h"

typedef struct Leaf {
    ScoopObjectHeader header;
    uint64_t value;
} Leaf;

static const ScoopTypeDescriptor leaf_td = {
    .type_id = 332,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(Leaf),
                       .instance_alignment = _Alignof(Leaf)}};

typedef enum Role { CONTROLLER, TARGET, COLLECTOR, ENTRANT } Role;
typedef struct Probe {
    Leaf *original;
    ScoopThreadState *target;
    ScoopManagedAnchor *anchor;
    uint64_t epoch;
    _Atomic(unsigned) ready, target_gate, request, waiting, stopped, collector_gate;
    _Atomic(unsigned) acknowledgements[2], wakes, held;
    _Atomic(bool) entrant_waiting, attached;
    bool passed;
} Probe;

static Probe *active;
static _Thread_local Role role;
static _Thread_local unsigned collector_round;

static void await(_Atomic(unsigned) *value, unsigned expected) {
    while (atomic_load_explicit(value, memory_order_acquire) < expected) {
        sched_yield();
    }
}

static void publish(_Atomic(unsigned) *value, unsigned next) {
    atomic_store_explicit(value, next, memory_order_release);
}

void scoop_thread_test_point(ScoopThreadTestPoint point) {
    Probe *probe = active;
    if (probe == NULL) {
        return;
    }
    if (role == ENTRANT && point == SCOOP_TEST_WORLD_WAITING) {
        atomic_store_explicit(&probe->entrant_waiting, true, memory_order_release);
    }
    if (role == TARGET) {
        if (point == SCOOP_TEST_PARKED_ACKNOWLEDGED) {
            uint64_t epoch = atomic_load(&probe->target->poll.observed_gc_epoch);
            assert(epoch > probe->epoch && epoch <= probe->epoch + 2);
            atomic_fetch_add(&probe->acknowledgements[epoch - probe->epoch - 1], 1);
            if (probe->anchor == NULL) {
                probe->anchor = probe->target->managed_anchor;
            }
            assert(probe->anchor != NULL && probe->anchor == probe->target->managed_anchor);
            assert(probe->target->parked_from == SCOOP_THREAD_MANAGED);
        } else if (point == SCOOP_TEST_WORLD_WOKEN) {
            atomic_fetch_add_explicit(&probe->wakes, 1, memory_order_release);
            if (atomic_load(&scoop_thread_world_phase) == SCOOP_WORLD_RUNNING &&
                atomic_load(&probe->held) == 0) {
                /* Let a second collector win the lock before this old parker
                 * has observed RUNNING in its wait predicate. */
                scoop_thread_registry_unlock();
                publish(&probe->held, 1);
                await(&probe->target_gate, 2);
                scoop_thread_registry_lock();
            }
        }
    } else if (role == COLLECTOR) {
        if (point == SCOOP_TEST_COLLECTOR_WAITING) {
            publish(&probe->waiting, collector_round + 1);
        } else if (point == SCOOP_TEST_COLLECTOR_STOPPED) {
            /* Collection has not started scanning yet. Keep roots frozen while
             * the controller injects unrelated notifications. */
            scoop_thread_registry_unlock();
            publish(&probe->stopped, collector_round + 1);
            await(&probe->collector_gate, collector_round + 1);
            scoop_thread_registry_lock();
        }
    }
}

static void *run_target(void *data) {
    Probe *probe = data;
    role = TARGET;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    probe->target = scoop_thread_current_required();
    _Alignas(16) uintptr_t frame[4] = {(uintptr_t)probe->original, 0, boundary, 0};
    publish(&probe->ready, 1);
    await(&probe->target_gate, 1);
    scoop_rt_safepoint_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    Leaf *root = (Leaf *)frame[0];
    probe->passed = root != probe->original && root->value == 42 &&
                    scoop_rt_gc_debug_is_allocated(root) &&
                    !scoop_rt_gc_debug_is_allocated(probe->original);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *run_collector(void *data) {
    Probe *probe = data;
    role = COLLECTOR;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
    for (collector_round = 0; collector_round < 2; collector_round++) {
        await(&probe->request, collector_round + 1);
        scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    }
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *run_entrant(void *data) {
    Probe *probe = data;
    role = ENTRANT;
    assert(scoop_rt_attach_foreign_thread());
    atomic_store_explicit(&probe->attached, true, memory_order_release);
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void unrelated_wakes(Probe *probe, unsigned round) {
    for (unsigned repeat = 0; repeat < 3; repeat++) {
        scoop_thread_registry_lock();
        unsigned wakes = atomic_load(&probe->wakes);
        scoop_thread_world_broadcast();
        scoop_thread_registry_unlock();
        await(&probe->wakes, wakes + 1);
        /* Taking the lock again also waits for the parker to recheck and wait. */
        scoop_thread_registry_lock();
        assert(atomic_load(&probe->acknowledgements[round]) == 1);
        assert(atomic_load(&probe->target->poll.mode) == SCOOP_THREAD_PARKED);
        assert(probe->anchor == probe->target->managed_anchor);
        scoop_thread_registry_unlock();
    }
}

static void run_scenario(void) {
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    Leaf *original = scoop_gc_alloc_internal(&leaf_td, sizeof *original);
    original->value = 42;
    scoop_thread_pop_managed_anchor(&anchor);
    scoop_thread_leave_managed();
    Probe probe = {.original = original, .epoch = atomic_load(&scoop_thread_gc_epoch)};
    active = &probe;
    pthread_t target, collector, entrant;
    assert(pthread_create(&target, NULL, run_target, &probe) == 0);
    await(&probe.ready, 1);
    assert(pthread_create(&collector, NULL, run_collector, &probe) == 0);
    publish(&probe.request, 1);
    await(&probe.waiting, 1);
    publish(&probe.target_gate, 1);
    await(&probe.stopped, 1);
    assert(atomic_load(&probe.acknowledgements[0]) == 1);
    unrelated_wakes(&probe, 0);
    assert(pthread_create(&entrant, NULL, run_entrant, &probe) == 0);
    while (!atomic_load_explicit(&probe.entrant_waiting, memory_order_acquire)) {
        sched_yield();
    }
    assert(!atomic_load(&probe.attached));
    publish(&probe.collector_gate, 1);
    await(&probe.held, 1);
    publish(&probe.request, 2);
    await(&probe.waiting, 2);
    assert(atomic_load(&probe.target->poll.mode) == SCOOP_THREAD_PARKED);
    assert(atomic_load(&probe.target->poll.observed_gc_epoch) == probe.epoch + 1);
    publish(&probe.target_gate, 2);
    await(&probe.stopped, 2);
    assert(atomic_load(&probe.acknowledgements[1]) == 1);
    unrelated_wakes(&probe, 1);
    publish(&probe.collector_gate, 2);
    assert(pthread_join(target, NULL) == 0);
    assert(pthread_join(collector, NULL) == 0);
    assert(pthread_join(entrant, NULL) == 0);
    assert(probe.passed && atomic_load(&probe.attached));
    active = NULL;
}

int main(void) {
    alarm(30);
    assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
    scoop_thread_runtime_init();
    const ScoopTypeDescriptor *types[] = {&leaf_td};
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    for (unsigned repeat = 0; repeat < 8; repeat++) {
        run_scenario();
    }
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("parked epochs, unrelated wakes, attachment and moving roots passed");
    return 0;
}
