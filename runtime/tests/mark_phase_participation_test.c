#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "../src/gc/mark_internal.h"
#include "../src/platform/platform.h"

#undef pthread_cond_wait
extern int pthread_cond_wait(pthread_cond_t *condition, pthread_mutex_t *lock);

static pthread_mutex_t gate = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t changed = PTHREAD_COND_INITIALIZER;
static _Atomic uint64_t delayed_phase;
static bool release_waiters;
static size_t blocked;
static size_t reparked;
static _Thread_local uint64_t blocked_phase;
static _Thread_local uint64_t reparked_phase;

int scoop_test_phase_wait(pthread_cond_t *condition, pthread_mutex_t *lock) {
    bool phase_wait = condition == &scoop_gc_marker.phase_changed;
    if (phase_wait) {
        pthread_mutex_lock(&gate);
        if (release_waiters && scoop_gc_marker.phase == atomic_load(&delayed_phase) &&
            reparked_phase != scoop_gc_marker.phase) {
            reparked_phase = scoop_gc_marker.phase;
            reparked++;
            pthread_cond_broadcast(&changed);
        }
        pthread_mutex_unlock(&gate);
    }
    int result = pthread_cond_wait(condition, lock);
    if (phase_wait && scoop_gc_marker.phase == atomic_load(&delayed_phase) &&
        blocked_phase != scoop_gc_marker.phase) {
        blocked_phase = scoop_gc_marker.phase;
        /* Delay inactive workers after the single-worker phase was published,
         * without retaining the pool lock or changing its visible state. */
        pthread_mutex_unlock(lock);
        pthread_mutex_lock(&gate);
        blocked++;
        pthread_cond_broadcast(&changed);
        while (!release_waiters) {
            pthread_cond_wait(&changed, &gate);
        }
        pthread_mutex_unlock(&gate);
        pthread_mutex_lock(lock);
    }
    return result;
}

static void change_participants(void) {
    pthread_mutex_lock(&gate);
    blocked = reparked = 0;
    release_waiters = false;
    atomic_store(&delayed_phase, scoop_gc_marker.phase + 1);
    pthread_mutex_unlock(&gate);

    scoop_gc_mark_pool_prepare(true);
    assert(scoop_gc_marker.active_workers == 1);
    scoop_gc_mark_pool_run(0);
    pthread_mutex_lock(&gate);
    while (blocked != 3) {
        pthread_cond_wait(&changed, &gate);
    }
    pthread_mutex_unlock(&gate);

    /* Prepare the next collection, but do not publish its first phase yet. */
    scoop_gc_mark_pool_prepare(false);
    assert(scoop_gc_marker.active_workers == 4);
    pthread_mutex_lock(&gate);
    release_waiters = true;
    pthread_cond_broadcast(&changed);
    while (reparked != 3) {
        pthread_cond_wait(&changed, &gate);
    }
    pthread_mutex_unlock(&gate);
    pthread_mutex_lock(&scoop_gc_marker.lock);
    assert(scoop_gc_marker.finished_workers == 0);
    pthread_mutex_unlock(&scoop_gc_marker.lock);
    atomic_store(&delayed_phase, 0);
    scoop_gc_mark_pool_run(0);
}

static void check_default_participants(void) {
    assert(unsetenv("SCOOP_GC_WORKERS") == 0);
    size_t expected = scoop_platform_bundle()->thread_vm->processor_count();
    if (expected > GC_MARK_DEFAULT_MAX_WORKERS) {
        expected = GC_MARK_DEFAULT_MAX_WORKERS;
    }
    committed_bytes = (size_t)3 << 20;
    scoop_gc_mark_pool_prepare(false);
    assert(!scoop_gc_marker.forced && scoop_gc_marker.configured_workers == expected);
    assert(scoop_gc_marker.active_workers == 1);
    scoop_gc_mark_pool_run(0);
    committed_bytes = (size_t)8 << 20;
    for (size_t iteration = 0; iteration < 3; iteration++) {
        scoop_gc_mark_pool_prepare(false);
        assert(scoop_gc_marker.active_workers == expected);
        scoop_gc_mark_pool_run(0);
        scoop_gc_mark_pool_prepare(true);
        assert(scoop_gc_marker.active_workers == 1);
        scoop_gc_mark_pool_run(0);
    }
    scoop_gc_shutdown();
}

int main(void) {
    alarm(20);
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
    committed_bytes = (size_t)8 << 20;
    scoop_gc_mark_pool_prepare(false);
    scoop_gc_marker.forced = false;
    scoop_gc_mark_pool_run(0);
    for (size_t iteration = 0; iteration < 3; iteration++) {
        change_participants();
    }
    scoop_gc_shutdown();
    check_default_participants();
    puts("delayed inactive workers retain the participants of their published phase");
}
