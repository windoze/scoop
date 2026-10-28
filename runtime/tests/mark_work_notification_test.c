#include <assert.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "../src/gc/mark_internal.h"

#undef pthread_cond_signal
#undef pthread_cond_broadcast
extern int pthread_cond_signal(pthread_cond_t *condition);
extern int pthread_cond_broadcast(pthread_cond_t *condition);

static _Atomic size_t work_signals;
static _Atomic size_t work_broadcasts;

int scoop_test_mark_signal(pthread_cond_t *condition) {
    if (condition == &scoop_gc_marker.work_available) {
        atomic_fetch_add(&work_signals, 1);
    }
    return pthread_cond_signal(condition);
}

int scoop_test_mark_broadcast(pthread_cond_t *condition) {
    if (condition == &scoop_gc_marker.work_available) {
        atomic_fetch_add(&work_broadcasts, 1);
    }
    return pthread_cond_broadcast(condition);
}

static void *run_phase(void *context) {
    (void)context;
    scoop_gc_mark_pool_run(0);
    return NULL;
}

static void wait_idle(void) {
    while (atomic_load(&scoop_gc_marker.pending) != 2 ||
           atomic_load(&scoop_gc_marker.waiting_workers) != GC_MARK_MAX_WORKERS) {
        sched_yield();
    }
}

int main(void) {
    alarm(30);
    char setting[16];
    snprintf(setting, sizeof setting, "%d", GC_MARK_MAX_WORKERS);
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    scoop_gc_mark_pool_prepare(false);
    assert(scoop_gc_marker.active_workers == GC_MARK_MAX_WORKERS);

    /* Publishing roots before a phase has no waiting consumers to notify. */
    for (size_t index = 0; index < 64; index++) {
        scoop_gc_mark_queue_push(&scoop_gc_marker.workers[0], (ScoopGcMarkTask){0});
    }
    assert(atomic_load(&work_signals) == 0);
    assert(atomic_load(&work_broadcasts) == 0);
    scoop_gc_mark_pool_run(0);
    assert(atomic_load(&scoop_gc_marker.pending) == 0);
    assert(atomic_load(&scoop_gc_marker.waiting_workers) == 0);

    for (size_t phase = 0; phase < 32; phase++) {
        size_t signals = atomic_load(&work_signals);
        size_t broadcasts = atomic_load(&work_broadcasts);
        /* Two outstanding producers keep the phase open and require waking
         * peers; a lone publisher is allowed to consume its own successor. */
        atomic_store(&scoop_gc_marker.pending, 2);
        pthread_t runner;
        assert(pthread_create(&runner, NULL, run_phase, NULL) == 0);
        wait_idle();
        for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
            scoop_gc_mark_queue_push(&scoop_gc_marker.workers[index], (ScoopGcMarkTask){0});
            wait_idle();
            assert(atomic_load(&work_signals) == signals + index + 1);
            assert(atomic_load(&work_broadcasts) == broadcasts);
        }
        scoop_gc_mark_task_done();
        scoop_gc_mark_task_done();
        assert(pthread_join(runner, NULL) == 0);
        assert(atomic_load(&work_broadcasts) == broadcasts + 1);
        assert(atomic_load(&scoop_gc_marker.pending) == 0);
        assert(atomic_load(&scoop_gc_marker.waiting_workers) == 0);
    }
    scoop_gc_shutdown();
    puts("new work wakes one waiting marker; completion releases every phase participant");
}
