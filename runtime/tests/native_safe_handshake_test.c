#include <assert.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
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
    .type_id = 331,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
                       .minimum_size = sizeof(Leaf),
                       .instance_alignment = _Alignof(Leaf)},
    .diagnostic_name = {(const uint8_t *)"NativeSafeLeaf", sizeof("NativeSafeLeaf") - 1},
};
static const uint64_t one_root_scan[] = {1, 0};

typedef enum Scenario { RETURN_DURING_SCAN, RETURN_BEFORE_STOP, SILENT_PUBLICATION } Scenario;
typedef enum Role { CONTROLLER, TARGET, COLLECTOR } Role;
typedef struct Probe {
    Scenario scenario;
    Leaf *original;
    ScoopThreadState *target;
    _Atomic(unsigned) target_stage, target_gate, collector_stage, collector_gate;
    _Atomic(bool) collected;
    bool passed;
} Probe;

static Probe *active;
static _Thread_local Role role;
static _Thread_local bool held_wait;

static void await(_Atomic(unsigned) *value, unsigned expected) {
    while (atomic_load_explicit(value, memory_order_acquire) < expected) {
        sched_yield();
    }
}

static void reach(_Atomic(unsigned) *stage, unsigned value) {
    atomic_store_explicit(stage, value, memory_order_release);
}

void scoop_thread_test_point(ScoopThreadTestPoint point) {
    Probe *probe = active;
    if (probe == NULL || role == CONTROLLER) {
        return;
    }
    if (role == TARGET) {
        if (point == SCOOP_TEST_NATIVE_SAFE_PUBLISHED) {
            unsigned stage = probe->scenario == SILENT_PUBLICATION ? 2 : 1;
            reach(&probe->target_stage, stage);
            await(&probe->target_gate, stage);
        } else if (probe->scenario == RETURN_DURING_SCAN &&
                   point == SCOOP_TEST_NATIVE_RETURNING_PUBLISHED && !held_wait) {
            held_wait = true;
            reach(&probe->target_stage, 2);
            await(&probe->target_gate, 2);
        } else if (probe->scenario == RETURN_DURING_SCAN &&
                   point == SCOOP_TEST_NATIVE_RETURNING_BLOCKED) {
            reach(&probe->target_stage, 3);
            await(&probe->target_gate, 3);
        } else if (probe->scenario == RETURN_BEFORE_STOP &&
                   point == SCOOP_TEST_NATIVE_RUNNING_OBSERVED && !held_wait) {
            held_wait = true;
            reach(&probe->target_stage, 2);
            await(&probe->target_gate, 2);
        }
        return;
    }
    if (probe->scenario == RETURN_DURING_SCAN) {
        if (point == SCOOP_TEST_COLLECTOR_STOPPED) {
            reach(&probe->collector_stage, 1);
            await(&probe->collector_gate, 1);
            assert(atomic_load(&probe->target->mode) == SCOOP_THREAD_NATIVE_SAFE_RETURNING);
        } else if (point == SCOOP_TEST_COLLECTOR_RESUMING) {
            reach(&probe->collector_stage, 2);
            await(&probe->collector_gate, 2);
        }
    } else if (point == SCOOP_TEST_COLLECTOR_WAITING && !held_wait) {
        held_wait = true;
        reach(&probe->collector_stage, 1);
        await(&probe->collector_gate, 1);
    }
}

static void *run_target(void *data) {
    Probe *probe = data;
    role = TARGET;
    assert(scoop_rt_attach_foreign_thread());
    probe->target = scoop_thread_current_required();
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    Leaf *root = probe->original;
    _Alignas(16) uintptr_t frame[4] = {(uintptr_t)root, 0, boundary, 0};
    ScoopCallerRootEntry entry = {.base = &root, .scan = one_root_scan};
    ScoopCallerRootFrame roots;
    scoop_rt_push_caller_roots(&roots, &entry, 1);
    if (probe->scenario == SILENT_PUBLICATION) {
        reach(&probe->target_stage, 1);
        await(&probe->target_gate, 1);
    }
    ScoopThreadTransition transition;
    scoop_rt_enter_native_safe_impl(&transition, (uintptr_t)frame, 0x1010, (uintptr_t)frame,
                                    (uintptr_t)&frame[2]);
    scoop_rt_leave_native_safe(&transition);
    scoop_rt_pop_caller_roots(&roots);
    /* The return-first case must publish another real safepoint before the
     * collector can scan the now-active segment. */
    scoop_rt_safepoint_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    root = (Leaf *)frame[0];
    probe->passed = root != probe->original && root->value == 7331 &&
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
    _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
    scoop_thread_enter_managed((void *)boundary);
#ifdef TEST_MINOR_GC
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    scoop_gc_collect_minor_internal();
    scoop_thread_pop_managed_anchor(&anchor);
#else
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
#endif
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    atomic_store_explicit(&probe->collected, true, memory_order_release);
    return NULL;
}

static void run_scenario(Scenario scenario) {
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    Leaf *root = scoop_gc_alloc_internal(&leaf_td, sizeof *root);
    root->value = 7331;
    scoop_thread_leave_managed();
    Probe probe = {.scenario = scenario, .original = root};
    active = &probe;
    pthread_t target, collector;
    assert(pthread_create(&target, NULL, run_target, &probe) == 0);
    await(&probe.target_stage, 1);
    if (scenario == RETURN_BEFORE_STOP) {
        reach(&probe.target_gate, 1);
        await(&probe.target_stage, 2);
    }
    assert(pthread_create(&collector, NULL, run_collector, &probe) == 0);
    await(&probe.collector_stage, 1);
    if (scenario == RETURN_DURING_SCAN) {
        reach(&probe.target_gate, 1);
        await(&probe.target_stage, 2);
        reach(&probe.collector_gate, 1);
        await(&probe.collector_stage, 2);
        reach(&probe.target_gate, 2);
        await(&probe.target_stage, 3);
        reach(&probe.collector_gate, 2);
        reach(&probe.target_gate, 3);
    } else if (scenario == RETURN_BEFORE_STOP) {
        assert(atomic_load(&scoop_thread_world_phase) == SCOOP_WORLD_STOPPING);
        assert(atomic_load(&probe.target->mode) == SCOOP_THREAD_NATIVE_SAFE_RETURNING);
        reach(&probe.target_gate, 2);
        reach(&probe.collector_gate, 1);
    } else {
        reach(&probe.target_gate, 1);
        await(&probe.target_stage, 2);
        /* Publication happened after the failed quiescence test. With no
         * broadcast from the target, only the timed recheck can make progress. */
        reach(&probe.collector_gate, 1);
        while (!atomic_load_explicit(&probe.collected, memory_order_acquire)) {
            sched_yield();
        }
        reach(&probe.target_gate, 2);
    }
    assert(pthread_join(target, NULL) == 0);
    assert(pthread_join(collector, NULL) == 0);
    assert(probe.passed);
    active = NULL;
}

int main(void) {
    alarm(30);
    scoop_thread_runtime_init();
    const ScoopTypeDescriptor *types[] = {&leaf_td};
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    for (unsigned repeat = 0; repeat < 20; repeat++) {
        run_scenario(RETURN_DURING_SCAN);
        run_scenario(RETURN_BEFORE_STOP);
        run_scenario(SILENT_PUBLICATION);
    }
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("native-safe return, publication and moving-root interleavings passed");
    return 0;
}
