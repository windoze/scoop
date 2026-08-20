#include "native.h"
#include <assert.h>
#include <errno.h>
#include <inttypes.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define SCOOP_THREAD_TESTING
#include "../../../runtime/src/thread/testing.h"

enum { MAX_THREADS = 8, MAX_PAUSES = 100000 };
typedef int64_t (*Worker)(int32_t, void *);
static Worker worker, collector;
static void *worker_context, *collector_context;
static unsigned threads;
static int32_t iterations, selected;
static _Atomic(unsigned) ready, finished;
static _Atomic(bool) start;
static uint64_t stop_started, waits[MAX_PAUSES], pauses[MAX_PAUSES];
static size_t pause_count;

static uint64_t now_ns(void) {
  struct timespec value;
  assert(clock_gettime(CLOCK_MONOTONIC, &value) == 0);
  return (uint64_t)value.tv_sec * UINT64_C(1000000000) +
         (uint64_t)value.tv_nsec;
}

int32_t bench_mode(void) {
  const char *mode = getenv("BENCH_MODE");
  selected = mode == NULL ? 2 : atoi(mode);
  assert(selected >= 0 && selected <= 8);
  return selected;
}

bool bench_done(void) {
  return atomic_load_explicit(&finished, memory_order_acquire) == threads;
}

void bench_pause(void) {
  const struct timespec interval = {.tv_nsec = 100000};
  assert(nanosleep(&interval, NULL) == 0);
}

void scoop_thread_test_point(ScoopThreadTestPoint point) {
  if (point != SCOOP_TEST_COLLECTOR_STOPPING &&
      point != SCOOP_TEST_COLLECTOR_STOPPED &&
      point != SCOOP_TEST_COLLECTOR_RESUMING) {
    return;
  }
  if (pause_count == MAX_PAUSES) {
    return;
  }
  if (point == SCOOP_TEST_COLLECTOR_STOPPING) {
    stop_started = now_ns();
  } else if (point == SCOOP_TEST_COLLECTOR_STOPPED) {
    waits[pause_count] = now_ns() - stop_started;
  } else if (point == SCOOP_TEST_COLLECTOR_RESUMING) {
    pauses[pause_count++] = now_ns() - stop_started;
  }
}

static int compare_ns(const void *left, const void *right) {
  uint64_t a = *(const uint64_t *)left, b = *(const uint64_t *)right;
  return (a > b) - (a < b);
}

static void distribution(const char *name, uint64_t *values) {
  qsort(values, pause_count, sizeof *values, compare_ns);
  printf(",\"%s\":{\"p50_ns\":%" PRIu64 ",\"p99_ns\":%" PRIu64
         ",\"max_ns\":%" PRIu64 "}",
         name, values[pause_count / 2], values[(pause_count - 1) * 99 / 100],
         values[pause_count - 1]);
}

static void await_start(void) {
  atomic_fetch_add_explicit(&ready, 1, memory_order_release);
  while (!atomic_load_explicit(&start, memory_order_acquire)) {
    sched_yield();
  }
}

static void *run_worker(void *unused) {
  (void)unused;
  await_start();
  int64_t sum = 0;
  if (selected == 0) {
    for (int32_t index = 0; index < iterations; index++) {
      sum += bench_scalar(index);
    }
  } else if (selected == 3) {
    for (int32_t index = 0; index < iterations; index++) {
      BenchPair result = bench_pair((BenchPair){index, index});
      sum += result.first + result.second;
    }
  } else if (selected == 6) {
    for (int32_t index = 0; index < iterations; index++) {
      errno = 0;
      int64_t result = bench_scalar(index);
      int error = errno;
      sum += result + error;
    }
  } else {
    sum = worker(iterations, worker_context);
  }
  int64_t count = iterations;
  bool scalar = selected < 3 || selected >= 6;
  int64_t expected = scalar ? count * (count + 1) / 2 : count * (count + 2);
  assert(sum == expected);
  atomic_fetch_add_explicit(&finished, 1, memory_order_release);
  return NULL;
}

static void *run_collector(void *unused) {
  (void)unused;
  await_start();
  assert(collector(0, collector_context) == 0);
  return NULL;
}

void bench_run(Worker callback, void *context, Worker gc_callback,
               void *gc_context) {
  worker = callback;
  worker_context = context;
  collector = gc_callback;
  collector_context = gc_context;
  threads = getenv("BENCH_THREADS") == NULL
                ? 1
                : (unsigned)atoi(getenv("BENCH_THREADS"));
  iterations = getenv("BENCH_ITERATIONS") == NULL
                   ? 2000000
                   : atoi(getenv("BENCH_ITERATIONS"));
  bool collect = getenv("BENCH_GC") != NULL;
  assert(threads > 0 && threads <= MAX_THREADS && iterations > 0);
  pthread_t workers[MAX_THREADS], gc;
  for (unsigned index = 0; index < threads; index++) {
    assert(pthread_create(&workers[index], NULL, run_worker, NULL) == 0);
  }
  if (collect) {
    assert(pthread_create(&gc, NULL, run_collector, NULL) == 0);
  }
  while (atomic_load_explicit(&ready, memory_order_acquire) !=
         threads + collect) {
    sched_yield();
  }
  uint64_t started = now_ns();
  atomic_store_explicit(&start, true, memory_order_release);
  for (unsigned index = 0; index < threads; index++) {
    assert(pthread_join(workers[index], NULL) == 0);
  }
  if (collect) {
    assert(pthread_join(gc, NULL) == 0);
  }
  uint64_t elapsed = now_ns() - started;
  printf("{\"mode\":%d,\"threads\":%u,\"iterations\":%d,\"gc\":%s,\"elapsed_"
         "ns\":%" PRIu64 ",\"ns_per_call\":%.3f,\"collections\":%zu",
         selected, threads, iterations, collect ? "true" : "false", elapsed,
         (double)elapsed / ((double)iterations * threads), pause_count);
  if (pause_count != 0) {
    distribution("stop_wait", waits);
    distribution("pause", pauses);
  }
  puts("}");
}
