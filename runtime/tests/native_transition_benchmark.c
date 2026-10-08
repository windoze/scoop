#include <assert.h>
#include <inttypes.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread/internal.h"
#include "platform/image_fixture.h"

void scoop_test_native_leaf(void);

enum { MAX_THREADS = 8, MAX_PAUSES = 100000 };
typedef enum CallMode { DIRECT, NATIVE_SAFE, NATIVE_BORROWED } CallMode;

static _Atomic(unsigned) ready_count, finished_count;
static _Atomic(bool) start;
static unsigned thread_count;
static uint64_t iterations;
static CallMode call_mode;
static bool collect;
#ifdef SCOOP_THREAD_TESTING
static uint64_t stop_started, wait_times[MAX_PAUSES], pause_times[MAX_PAUSES];
static size_t pause_count;
#endif

static uint64_t now_ns(void) {
    struct timespec value;
    assert(clock_gettime(CLOCK_MONOTONIC, &value) == 0);
    return (uint64_t)value.tv_sec * UINT64_C(1000000000) + (uint64_t)value.tv_nsec;
}

#ifdef SCOOP_THREAD_TESTING
void scoop_thread_test_point(ScoopThreadTestPoint point) {
    if (point != SCOOP_TEST_COLLECTOR_STOPPING && point != SCOOP_TEST_COLLECTOR_STOPPED &&
        point != SCOOP_TEST_COLLECTOR_RESUMING) {
        return;
    }
    if (pause_count == MAX_PAUSES) {
        return;
    }
    switch (point) {
    case SCOOP_TEST_COLLECTOR_STOPPING:
        stop_started = now_ns();
        break;
    case SCOOP_TEST_COLLECTOR_STOPPED:
        wait_times[pause_count] = now_ns() - stop_started;
        break;
    case SCOOP_TEST_COLLECTOR_RESUMING:
        pause_times[pause_count++] = now_ns() - stop_started;
        break;
    default:
        break;
    }
}

static int compare_ns(const void *left, const void *right) {
    uint64_t a = *(const uint64_t *)left, b = *(const uint64_t *)right;
    return (a > b) - (a < b);
}

static void print_distribution(const char *name, uint64_t *values) {
    qsort(values, pause_count, sizeof *values, compare_ns);
    printf(",\"%s\":{\"p50_ns\":%" PRIu64 ",\"p99_ns\":%" PRIu64 ",\"max_ns\":%" PRIu64 "}", name,
           values[pause_count / 2], values[(pause_count - 1) * 99 / 100], values[pause_count - 1]);
}
#endif

static void *run_worker(void *unused) {
    (void)unused;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
    atomic_fetch_add_explicit(&ready_count, 1, memory_order_release);
    while (!atomic_load_explicit(&start, memory_order_acquire)) {
        sched_yield();
    }
    scoop_thread_enter_managed((void *)boundary);
    for (uint64_t iteration = 0; iteration < iterations; iteration++) {
        if (call_mode == DIRECT) {
            scoop_test_native_leaf();
        } else {
            ScoopCallerRootFrame roots;
            ScoopThreadTransition transition;
            scoop_rt_push_caller_roots(&roots, NULL, 0);
            if (call_mode == NATIVE_SAFE) {
                scoop_rt_enter_native_safe_impl(&transition, (uintptr_t)frame, 0x1010,
                                                (uintptr_t)frame, (uintptr_t)&frame[2]);
                scoop_test_native_leaf();
                scoop_rt_leave_native_safe(&transition);
            } else {
                scoop_rt_enter_native_borrowed_impl(&transition, (uintptr_t)frame, 0x1010,
                                                    (uintptr_t)frame, (uintptr_t)&frame[2]);
                scoop_test_native_leaf();
                scoop_rt_leave_native_borrowed(&transition);
            }
            scoop_rt_pop_caller_roots(&roots);
        }
        if (collect) {
            scoop_rt_safepoint_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
        }
    }
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    atomic_fetch_add_explicit(&finished_count, 1, memory_order_release);
    return NULL;
}

int main(int argc, char **argv) {
    assert(argc == 4 || argc == 5);
    assert(strcmp(argv[1], "direct") == 0 || strcmp(argv[1], "safe") == 0 ||
           strcmp(argv[1], "borrowed") == 0);
    call_mode = strcmp(argv[1], "direct") == 0 ? DIRECT
                : strcmp(argv[1], "safe") == 0 ? NATIVE_SAFE
                                               : NATIVE_BORROWED;
    thread_count = (unsigned)strtoul(argv[2], NULL, 10);
    iterations = strtoull(argv[3], NULL, 10);
    collect = argc == 5 && strcmp(argv[4], "gc") == 0;
    assert(thread_count > 0 && thread_count <= MAX_THREADS && iterations > 0);
    scoop_thread_runtime_init();
    scoop_test_image_init(NULL, 0, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    pthread_t workers[MAX_THREADS];
    for (unsigned index = 0; index < thread_count; index++) {
        assert(pthread_create(&workers[index], NULL, run_worker, NULL) == 0);
    }
    while (atomic_load_explicit(&ready_count, memory_order_acquire) != thread_count) {
        sched_yield();
    }
    uint64_t started = now_ns();
    atomic_store_explicit(&start, true, memory_order_release);
    if (collect) {
        uintptr_t boundary = scoop_rt_thread_debug_stack_high();
        _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
        scoop_thread_enter_managed((void *)boundary);
        while (atomic_load_explicit(&finished_count, memory_order_acquire) != thread_count) {
            scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
            /* Give mutators a fixed interval between requests. A tight loop
             * can repeatedly win the world lock and measures starvation. */
            const struct timespec interval = {.tv_nsec = 100000};
            assert(nanosleep(&interval, NULL) == 0);
        }
        scoop_thread_leave_managed();
    }
    for (unsigned index = 0; index < thread_count; index++) {
        assert(pthread_join(workers[index], NULL) == 0);
    }
    uint64_t elapsed = now_ns() - started;
    printf("{\"mode\":\"%s\",\"threads\":%u,\"iterations\":%" PRIu64
           ",\"gc\":%s,\"elapsed_ns\":%" PRIu64 ",\"ns_per_call\":%.3f",
           argv[1], thread_count, iterations, collect ? "true" : "false", elapsed,
           (double)elapsed / (double)(iterations * thread_count));
#ifdef SCOOP_THREAD_TESTING
    printf(",\"collections\":%zu", pause_count);
    if (pause_count != 0) {
        print_distribution("stop_wait", wait_times);
        print_distribution("pause", pause_times);
    }
#endif
    puts("}");
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    return 0;
}
