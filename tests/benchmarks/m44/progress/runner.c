#define _POSIX_C_SOURCE 200809L
#include "progress.h"
#include "scoop_rt.h"
#include <assert.h>
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stdlib.h>
#include <time.h>

typedef int64_t (*Callback)(int32_t, void *);
unsigned m44_threads;
int32_t m44_iterations;
static int mode;
static Callback worker, collector;
static void *worker_context, *collector_context;
static _Atomic(unsigned) ready, finished;
static _Atomic(bool) start;

static int environment_integer(const char *name, int fallback) {
    const char *text = getenv(name);
    if (text == NULL) {
        return fallback;
    }
    char *end;
    long value = strtol(text, &end, 10);
    assert(*text != 0 && *end == 0 && value >= 0 && value <= INT32_MAX);
    return (int)value;
}

int32_t m44_progress_mode(void) {
    mode = environment_integer("BENCH_MODE", 0);
    assert(mode == 0 || mode == 1);
    return mode;
}

bool m44_progress_done(void) {
    return atomic_load_explicit(&finished, memory_order_acquire) == m44_threads;
}

void m44_progress_pause(void) {
    struct timespec interval = {.tv_nsec = 100000};
    while (nanosleep(&interval, &interval) != 0) {
        assert(errno == EINTR);
    }
}

static void await_start(void) {
    atomic_fetch_add_explicit(&ready, 1, memory_order_release);
    while (!atomic_load_explicit(&start, memory_order_acquire)) {
        sched_yield();
    }
}

static void *run_worker(void *argument) {
    m44_worker_index = (unsigned)(uintptr_t)argument;
    await_start();
    int64_t sum = worker(m44_iterations, worker_context);
    int64_t count = m44_iterations;
    assert(sum == count * (count + 1) / 2);
    assert(m44_workers[m44_worker_index].checksum == sum);
    atomic_fetch_add_explicit(&finished, 1, memory_order_release);
    return NULL;
}

static void *run_collector(void *unused) {
    (void)unused;
    await_start();
    assert(collector(0, collector_context) == 0);
    return NULL;
}

void m44_progress_run(Callback callback, void *context, Callback gc_callback, void *gc_context) {
    worker = callback;
    worker_context = context;
    collector = gc_callback;
    collector_context = gc_context;
    m44_threads = (unsigned)environment_integer("BENCH_THREADS", 4);
    m44_iterations = environment_integer("BENCH_ITERATIONS", mode == 0 ? 100000000 : 2000000);
    assert(m44_threads > 0 && m44_threads <= M44_MAX_THREADS && m44_iterations > 0);
    m44_observation_init();
    ScoopGcMetrics before, after;
    scoop_rt_gc_debug_metrics(&before);
    pthread_t workers[M44_MAX_THREADS], gc;
    for (unsigned index = 0; index < m44_threads; index++) {
        assert(pthread_create(&workers[index], NULL, run_worker, (void *)(uintptr_t)index) == 0);
    }
    bool collect = mode == 0;
    if (collect) {
        assert(pthread_create(&gc, NULL, run_collector, NULL) == 0);
    }
    while (atomic_load_explicit(&ready, memory_order_acquire) != m44_threads + collect) {
        sched_yield();
    }
    uint64_t started = m44_now_ns();
    atomic_store_explicit(&start, true, memory_order_release);
    for (unsigned index = 0; index < m44_threads; index++) {
        assert(pthread_join(workers[index], NULL) == 0);
    }
    if (collect) {
        assert(pthread_join(gc, NULL) == 0);
    }
    uint64_t elapsed = m44_now_ns() - started;
    scoop_rt_gc_debug_metrics(&after);
    m44_report(mode, elapsed, after.allocated_bytes - before.allocated_bytes);
}
