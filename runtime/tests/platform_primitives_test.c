#if defined(_WIN32)
#include <windows.h>
#endif
#include "platform/primitives.h"
#include "scoop_runtime_metadata_v1.h"
#include <assert.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

enum { WORKERS = 4, REPETITIONS = 10000 };
static ScoopPlatformMutex mutex = SCOOP_PLATFORM_MUTEX_INIT;
static ScoopPlatformCondition ready_changed = SCOOP_PLATFORM_CONDITION_INIT;
static ScoopPlatformCondition start_changed = SCOOP_PLATFORM_CONDITION_INIT;
static ScoopPlatformCondition finished_changed = SCOOP_PLATFORM_CONDITION_INIT;
static ScoopPlatformOnce once = SCOOP_PLATFORM_ONCE_INIT;
static unsigned ready, finished, total, initialized;
static bool started;
static _Thread_local uintptr_t local_value;

static void initialize(void) { initialized++; }

static void *worker(void *argument) {
    assert(scoop_platform_mutex_lock(&mutex) == 0);
    ready++;
    assert(scoop_platform_condition_signal(&ready_changed) == 0);
    while (!started)
        assert(scoop_platform_condition_wait(&start_changed, &mutex) == 0);
    assert(scoop_platform_mutex_unlock(&mutex) == 0);
    assert(scoop_platform_once(&once, initialize) == 0);
    assert(initialized == 1);
    local_value = (uintptr_t)argument;
    for (unsigned iteration = 0; iteration < REPETITIONS; ++iteration) {
        assert(scoop_platform_mutex_lock(&mutex) == 0);
        total++;
        assert(scoop_platform_mutex_unlock(&mutex) == 0);
        assert(local_value == (uintptr_t)argument);
    }
    assert(scoop_platform_mutex_lock(&mutex) == 0);
    finished++;
    assert(scoop_platform_condition_signal(&finished_changed) == 0);
    assert(scoop_platform_mutex_unlock(&mutex) == 0);
    return NULL;
}

static int run(void) {
    ScoopPlatformThread threads[WORKERS];
    uint64_t before, after, cpu_before, cpu_after;
    assert(scoop_platform_monotonic_ns(&before));
    assert(scoop_platform_thread_cpu_ns(&cpu_before));
    for (uintptr_t index = 0; index < WORKERS; ++index)
        assert(scoop_platform_thread_create(&threads[index], worker, (void *)(index + 1)) == 0);
    assert(scoop_platform_mutex_lock(&mutex) == 0);
    while (ready != WORKERS)
        assert(scoop_platform_condition_wait_for(&ready_changed, &mutex, 5000000) == 0);
    started = true;
    assert(scoop_platform_condition_broadcast(&start_changed) == 0);
    while (finished != WORKERS)
        assert(scoop_platform_condition_wait_for(&finished_changed, &mutex, 5000000) == 0);
    assert(total == WORKERS * REPETITIONS && initialized == 1 && local_value == 0);
    assert(scoop_platform_condition_wait_for(&finished_changed, &mutex, 50) == ETIMEDOUT);
    assert(scoop_platform_mutex_unlock(&mutex) == 0);
    for (unsigned index = 0; index < WORKERS; ++index)
        assert(scoop_platform_thread_join(threads[index]) == 0);
    assert(scoop_platform_monotonic_ns(&after) && after > before);
    assert(scoop_platform_thread_cpu_ns(&cpu_after) && cpu_after >= cpu_before);
    ScoopPlatformMutex dynamic;
    assert(scoop_platform_mutex_init(&dynamic) == 0);
    assert(scoop_platform_mutex_lock(&dynamic) == 0);
    assert(scoop_platform_mutex_unlock(&dynamic) == 0);
    scoop_platform_mutex_destroy(&dynamic);
    for (size_t alignment = 8; alignment <= 128; alignment *= 2) {
        void *memory = scoop_platform_aligned_allocate(alignment, 256);
        assert(memory != NULL && (uintptr_t)memory % alignment == 0);
        memset(memory, 0x5a, 256);
        scoop_platform_aligned_free(memory);
    }
    scoop_platform_mutex_destroy(&mutex);
    puts("parallel workers, conditions, once, TLS, clocks and aligned storage passed");
    return 0;
}

#if defined(_WIN32)
int wmain(int argc, uint16_t **argv) {
    (void)argc;
    (void)argv;
    return run();
}
#else
int main(void) { return run(); }
#endif
