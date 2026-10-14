#define _POSIX_C_SOURCE 200809L
#include "progress.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define SCOOP_THREAD_TESTING
#include "thread/testing.h"

typedef struct ProgressPair {
    uint64_t last_ns, last_completed, first_ns, first_completed;
} ProgressPair;

typedef struct Collection {
    uint64_t stopping, collecting, running;
    ProgressPair progress[M44_MAX_THREADS];
} Collection;

M44Worker m44_workers[M44_MAX_THREADS];
_Thread_local unsigned m44_worker_index;
static _Thread_local size_t next_observation;
static Collection *collections;
static _Atomic(size_t) published;
static _Atomic(uint64_t) nursery_retries;
static size_t collection_count;

uint64_t m44_now_ns(void) {
    struct timespec now;
    assert(clock_gettime(CLOCK_MONOTONIC, &now) == 0);
    return (uint64_t)now.tv_sec * UINT64_C(1000000000) + (uint64_t)now.tv_nsec;
}

void m44_observation_init(void) {
    collections = calloc(M44_MAX_COLLECTIONS, sizeof *collections);
    assert(collections != NULL);
}

void m44_progress_point(int64_t completed, int64_t checksum, bool finished) {
    assert(m44_worker_index < m44_threads && completed >= 0);
    M44Worker *worker = &m44_workers[m44_worker_index];
    assert(checksum == completed * (completed + 1) / 2);
    size_t count = atomic_load_explicit(&published, memory_order_acquire);
    uint64_t now = m44_now_ns();
    for (; next_observation < count; next_observation++) {
        Collection *collection = &collections[next_observation];
        ProgressPair *pair = &collection->progress[m44_worker_index];
        if (pair->last_ns != 0) {
            assert(now >= collection->running && (uint64_t)completed > pair->last_completed);
            pair->first_ns = now;
            pair->first_completed = (uint64_t)completed;
        }
    }
    worker->probes++;
    worker->checksum = checksum;
    atomic_store_explicit(&worker->completed, (uint64_t)completed, memory_order_relaxed);
    atomic_store_explicit(&worker->last_ns, now, memory_order_release);
    atomic_store_explicit(&worker->active, !finished, memory_order_release);
}

void scoop_thread_test_point(ScoopThreadTestPoint point) {
    if (point == SCOOP_TEST_NURSERY_RETRY) {
        atomic_fetch_add_explicit(&nursery_retries, 1, memory_order_relaxed);
        return;
    }
    if (collections == NULL ||
        (point != SCOOP_TEST_COLLECTOR_STOPPING && point != SCOOP_TEST_COLLECTOR_STOPPED &&
         point != SCOOP_TEST_COLLECTOR_RUNNING)) {
        return;
    }
    assert(collection_count < M44_MAX_COLLECTIONS);
    Collection *collection = &collections[collection_count];
    uint64_t now = m44_now_ns();
    if (point == SCOOP_TEST_COLLECTOR_STOPPING) {
        collection->stopping = now;
    } else if (point == SCOOP_TEST_COLLECTOR_STOPPED) {
        collection->collecting = now;
        for (unsigned index = 0; index < m44_threads; index++) {
            M44Worker *worker = &m44_workers[index];
            if (atomic_load_explicit(&worker->active, memory_order_acquire)) {
                ProgressPair *pair = &collection->progress[index];
                pair->last_ns = atomic_load_explicit(&worker->last_ns, memory_order_acquire);
                pair->last_completed =
                    atomic_load_explicit(&worker->completed, memory_order_relaxed);
            }
        }
    } else {
        collection->running = now;
        atomic_store_explicit(&published, ++collection_count, memory_order_release);
    }
}

void m44_report(int mode, uint64_t elapsed, uint64_t allocated_bytes) {
    printf("{\"scoop_progress\":1,\"mode\":%d,\"threads\":%u,\"iterations\":%d,"
           "\"probe_every\":%d,\"elapsed_ns\":%" PRIu64 ",\"allocated_bytes\":%" PRIu64
           ",\"nursery_retries\":%" PRIu64 ",\"workers\":[",
           mode, m44_threads, m44_iterations, M44_PROBE_EVERY, elapsed, allocated_bytes,
           atomic_load_explicit(&nursery_retries, memory_order_relaxed));
    for (unsigned index = 0; index < m44_threads; index++) {
        M44Worker *worker = &m44_workers[index];
        assert(!atomic_load_explicit(&worker->active, memory_order_acquire));
        printf("%s{\"probes\":%" PRIu64 ",\"checksum\":%" PRId64 "}", index ? "," : "",
               worker->probes, worker->checksum);
    }
    fputs("],\"collections\":[", stdout);
    for (size_t index = 0; index < collection_count; index++) {
        Collection *collection = &collections[index];
        assert(collection->stopping <= collection->collecting &&
               collection->collecting <= collection->running);
        printf("%s{\"stopping_ns\":%" PRIu64 ",\"collecting_ns\":%" PRIu64
               ",\"running_ns\":%" PRIu64 ",\"progress\":[",
               index ? "," : "", collection->stopping, collection->collecting, collection->running);
        for (unsigned thread = 0; thread < m44_threads; thread++) {
            ProgressPair *pair = &collection->progress[thread];
            assert(pair->last_ns == 0 || pair->first_ns >= collection->running);
            printf("%s[ %" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 " ]", thread ? "," : "",
                   pair->last_ns, pair->last_completed, pair->first_ns, pair->first_completed);
        }
        fputs("]}", stdout);
    }
    puts("]}");
    free(collections);
    collections = NULL;
}
