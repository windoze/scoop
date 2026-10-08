#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread/internal.h"
#include "platform/image_fixture.h"

typedef enum Scenario {
    BEFORE_ENTRY,
    PENDING_POLL,
    ACTIVE_POLL,
    RETURN_TO_NATIVE,
    BETWEEN_GATEWAYS,
    NESTED_CALLBACK
} Scenario;

typedef struct Rendezvous {
    pthread_mutex_t lock;
    pthread_cond_t changed;
    Scenario scenario;
    unsigned stage;
    bool proceed, detach;
    ScoopThreadState *target;
} Rendezvous;

static void frame_init(uintptr_t frame[4], const void *boundary) {
    memset(frame, 0, 4 * sizeof *frame);
    frame[2] = (uintptr_t)boundary;
}

static void poll_frame(uintptr_t frame[4]) {
    scoop_rt_safepoint_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
}

static void target_ready(Rendezvous *sync) {
    assert(pthread_mutex_lock(&sync->lock) == 0);
    sync->target = scoop_thread_current_required();
    sync->stage = 1;
    assert(pthread_cond_broadcast(&sync->changed) == 0);
    while (!sync->proceed)
        assert(pthread_cond_wait(&sync->changed, &sync->lock) == 0);
    sync->stage = 2;
    assert(pthread_cond_broadcast(&sync->changed) == 0);
    assert(pthread_mutex_unlock(&sync->lock) == 0);
}

static void nested_target(Rendezvous *sync, const void *boundary) {
    _Alignas(16) uintptr_t frames[3][4];
    frame_init(frames[2], boundary);
    frame_init(frames[1], frames[2]);
    frame_init(frames[0], frames[1]);
    ScoopCallerRootFrame roots[2] = {0};
    ScoopThreadTransition transitions[2] = {0};
    ScoopCallbackThreadEntry callbacks[2] = {0};
    scoop_thread_enter_managed(boundary);
    scoop_rt_push_caller_roots(&roots[0], NULL, 0);
    scoop_rt_enter_native_borrowed_impl(&transitions[0], (uintptr_t)frames[2], 0x1010,
                                        (uintptr_t)frames[2], (uintptr_t)&frames[2][2]);
    scoop_thread_enter_callback(&callbacks[0], frames[2]);
    scoop_rt_push_caller_roots(&roots[1], NULL, 0);
    scoop_rt_enter_native_safe_impl(&transitions[1], (uintptr_t)frames[1], 0x1010,
                                    (uintptr_t)frames[1], (uintptr_t)&frames[1][2]);
    scoop_thread_enter_callback(&callbacks[1], frames[1]);
    assert(scoop_thread_current_required()->callback_depth == 2);
    target_ready(sync);
    poll_frame(frames[0]);
    scoop_thread_leave_callback(&callbacks[1]);
    scoop_rt_leave_native_safe(&transitions[1]);
    assert(scoop_thread_current_required()->managed_stack_boundary == (const char *)frames[2]);
    scoop_rt_pop_caller_roots(&roots[1]);
    scoop_thread_leave_callback(&callbacks[0]);
    scoop_rt_leave_native_borrowed(&transitions[0]);
    assert(scoop_thread_current_required()->managed_stack_boundary == boundary);
    scoop_rt_pop_caller_roots(&roots[0]);
    scoop_thread_leave_managed();
}

static void *target_thread(void *context) {
    Rendezvous *sync = context;
    assert(scoop_rt_attach_foreign_thread());
    const void *boundary = __builtin_frame_address(0);
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame, boundary);
    if (sync->scenario == NESTED_CALLBACK) {
        nested_target(sync, boundary);
    } else {
        if (sync->scenario == BETWEEN_GATEWAYS) {
            scoop_thread_enter_gateway(boundary);
            poll_frame(frame);
            scoop_thread_leave_managed();
        }
        bool enter_later = sync->scenario == BEFORE_ENTRY || sync->scenario == BETWEEN_GATEWAYS;
        if (!enter_later) {
            scoop_thread_enter_gateway(boundary);
            if (sync->scenario != PENDING_POLL)
                poll_frame(frame);
        }
        target_ready(sync);
        if (enter_later)
            scoop_thread_enter_gateway(boundary);
        if (sync->scenario != RETURN_TO_NATIVE)
            poll_frame(frame);
        scoop_thread_leave_managed();
    }
    ScoopThreadState *state = scoop_thread_current_required();
    assert(state->managed_stack_boundary == NULL && state->managed_anchor == NULL);
    assert(atomic_load(&state->mode) == SCOOP_THREAD_NATIVE_SAFE && state->managed_depth == 0);
    assert(state->callback_depth == 0 && state->current_transition == NULL);
    assert(pthread_mutex_lock(&sync->lock) == 0);
    sync->stage = 3;
    assert(pthread_cond_broadcast(&sync->changed) == 0);
    while (!sync->detach)
        assert(pthread_cond_wait(&sync->changed, &sync->lock) == 0);
    assert(pthread_mutex_unlock(&sync->lock) == 0);
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *collector_thread(void *unused) {
    (void)unused;
    assert(scoop_rt_attach_foreign_thread());
    const void *boundary = __builtin_frame_address(0);
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame, boundary);
    scoop_thread_enter_managed(boundary);
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void wait_stage(Rendezvous *sync, unsigned stage) {
    assert(pthread_mutex_lock(&sync->lock) == 0);
    while (sync->stage < stage)
        assert(pthread_cond_wait(&sync->changed, &sync->lock) == 0);
    assert(pthread_mutex_unlock(&sync->lock) == 0);
}

static void proceed(Rendezvous *sync) {
    assert(pthread_mutex_lock(&sync->lock) == 0);
    sync->proceed = true;
    assert(pthread_cond_broadcast(&sync->changed) == 0);
    assert(pthread_mutex_unlock(&sync->lock) == 0);
}

static void wait_world(ScoopWorldPhase expected) {
    scoop_thread_registry_lock();
    while (atomic_load(&scoop_thread_world_phase) < expected)
        scoop_thread_world_wait();
    assert(atomic_load(&scoop_thread_world_phase) == expected);
    scoop_thread_registry_unlock();
}

static void run_scenario(Scenario scenario) {
    Rendezvous sync = {.lock = PTHREAD_MUTEX_INITIALIZER,
                       .changed = PTHREAD_COND_INITIALIZER,
                       .scenario = scenario};
    pthread_t target, collector;
    assert(pthread_create(&target, NULL, target_thread, &sync) == 0);
    wait_stage(&sync, 1);
    /* Hold tracing at the heap lock after the real STW handshake. No timing
     * delays or production test hooks determine these interleavings. */
    scoop_gc_heap_lock();
    assert(pthread_create(&collector, NULL, collector_thread, NULL) == 0);
    bool active =
        scenario == ACTIVE_POLL || scenario == RETURN_TO_NATIVE || scenario == NESTED_CALLBACK;
    wait_world(active ? SCOOP_WORLD_STOPPING : SCOOP_WORLD_COLLECTING);
    proceed(&sync);
    wait_stage(&sync, 2);
    if (scenario == PENDING_POLL) {
        scoop_thread_registry_lock();
        while (atomic_load(&sync.target->mode) != SCOOP_THREAD_PARKED)
            scoop_thread_world_wait();
        assert(sync.target->parked_from == SCOOP_THREAD_MANAGED_PENDING);
        assert(sync.target->managed_anchor == NULL);
        scoop_thread_registry_unlock();
    } else if (active) {
        wait_world(SCOOP_WORLD_COLLECTING);
        scoop_thread_registry_lock();
        if (scenario == RETURN_TO_NATIVE) {
            assert(atomic_load(&sync.target->mode) == SCOOP_THREAD_NATIVE_SAFE);
            assert(sync.target->managed_stack_boundary == NULL);
        } else {
            assert(atomic_load(&sync.target->mode) == SCOOP_THREAD_PARKED);
            assert(sync.target->parked_from == SCOOP_THREAD_MANAGED);
            assert(sync.target->managed_anchor != NULL);
        }
        scoop_thread_registry_unlock();
    } else {
        scoop_thread_registry_lock();
        assert(atomic_load(&sync.target->mode) == SCOOP_THREAD_NATIVE_SAFE);
        assert(sync.target->managed_stack_boundary == NULL);
        scoop_thread_registry_unlock();
    }
    scoop_gc_heap_unlock();
    assert(pthread_join(collector, NULL) == 0);
    wait_stage(&sync, 3);
    assert(pthread_mutex_lock(&sync.lock) == 0);
    sync.detach = true;
    assert(pthread_cond_broadcast(&sync.changed) == 0);
    assert(pthread_mutex_unlock(&sync.lock) == 0);
    assert(pthread_join(target, NULL) == 0);
    assert(pthread_mutex_destroy(&sync.lock) == 0);
    assert(pthread_cond_destroy(&sync.changed) == 0);
    assert(scoop_rt_thread_debug_count() == 1);
}

static void invalid_entry(unsigned test) {
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        struct rlimit limit = {0, 0};
        assert(setrlimit(RLIMIT_CORE, &limit) == 0);
        assert(freopen("/dev/null", "w", stderr) != NULL);
        _Alignas(16) uintptr_t frame[4];
        ScoopManagedAnchor anchor;
        const void *boundary = __builtin_frame_address(0);
        frame_init(frame, boundary);
        if (test == 2) {
            scoop_thread_enter_managed(boundary);
            scoop_thread_registry_lock();
            scoop_thread_park_current_locked(scoop_thread_current_required());
        } else {
            scoop_thread_enter_gateway(boundary);
            if (test == 0)
                scoop_thread_leave_managed();
            else
                scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame,
                                                 (uintptr_t)&frame[2]);
        }
        _exit(0);
    }
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
}

int main(void) {
    alarm(30); /* Deadlock watchdog; barriers above control every tested race. */
    scoop_thread_runtime_init();
    scoop_test_image_init(NULL, 0, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_NATIVE_SAFE);
    for (Scenario scenario = BEFORE_ENTRY; scenario <= NESTED_CALLBACK; scenario++)
        run_scenario(scenario);
    for (unsigned test = 0; test < 3; test++)
        invalid_entry(test);
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    alarm(0);
    puts("gateway entry and GC handshake tests passed");
}
