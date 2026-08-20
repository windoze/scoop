/* Internal persistent marker threads never attach to the mutator registry. */
#define _POSIX_C_SOURCE 200809L
#include <stdlib.h>

#include "../platform/platform.h"
#include "mark_internal.h"

ScoopGcMarker scoop_gc_marker = {
    .lock = PTHREAD_MUTEX_INITIALIZER,
    .phase_changed = PTHREAD_COND_INITIALIZER,
    .work_available = PTHREAD_COND_INITIALIZER,
    .finished = PTHREAD_COND_INITIALIZER,
};

void scoop_gc_mark_notify_work(void) {
    if (scoop_gc_marker.active_workers == 1) {
        return;
    }
    pthread_mutex_lock(&scoop_gc_marker.lock);
    pthread_cond_broadcast(&scoop_gc_marker.work_available);
    pthread_mutex_unlock(&scoop_gc_marker.lock);
}

static void run_worker(ScoopGcMarkWorker *worker) {
    uint64_t started = scoop_gc_thread_cpu_ns();
    for (;;) {
        ScoopGcMarkTask task;
        bool found = scoop_gc_mark_queue_take(worker, &task);
        if (!found) {
            pthread_mutex_lock(&scoop_gc_marker.lock);
            /* Search again under the wake lock so publication cannot be missed. */
            while (atomic_load_explicit(&scoop_gc_marker.pending, memory_order_acquire) != 0 &&
                   !(found = scoop_gc_mark_queue_take(worker, &task))) {
                pthread_cond_wait(&scoop_gc_marker.work_available, &scoop_gc_marker.lock);
            }
            pthread_mutex_unlock(&scoop_gc_marker.lock);
            if (!found) {
                break;
            }
        }
        scoop_gc_mark_run_task(worker, task);
        scoop_gc_mark_flush(worker);
        scoop_gc_mark_task_done();
    }
    worker->cpu_ns += scoop_gc_thread_cpu_ns() - started;
}

static void *worker_main(void *context) {
    ScoopGcMarkWorker *worker = context;
    uint64_t observed = worker->initial_phase;
    pthread_mutex_lock(&scoop_gc_marker.lock);
    for (;;) {
        while (!scoop_gc_marker.stopping && observed == scoop_gc_marker.phase) {
            pthread_cond_wait(&scoop_gc_marker.phase_changed, &scoop_gc_marker.lock);
        }
        if (scoop_gc_marker.stopping) {
            break;
        }
        observed = scoop_gc_marker.phase;
        if (worker->id >= scoop_gc_marker.active_workers) {
            continue;
        }
        pthread_mutex_unlock(&scoop_gc_marker.lock);
        run_worker(worker);
        pthread_mutex_lock(&scoop_gc_marker.lock);
        scoop_gc_marker.finished_workers++;
        pthread_cond_signal(&scoop_gc_marker.finished);
    }
    pthread_mutex_unlock(&scoop_gc_marker.lock);
    return NULL;
}

static void stop_threads(void) {
    pthread_mutex_lock(&scoop_gc_marker.lock);
    scoop_gc_marker.stopping = true;
    pthread_cond_broadcast(&scoop_gc_marker.phase_changed);
    pthread_mutex_unlock(&scoop_gc_marker.lock);
    for (size_t index = 1; index <= scoop_gc_marker.created_workers; index++) {
        if (pthread_join(scoop_gc_marker.workers[index].thread, NULL) != 0) {
            heap_fatal("failed to join a GC marker");
        }
    }
    scoop_gc_marker.created_workers = 0;
    scoop_gc_marker.stopping = false;
}

static void initialize(void) {
    size_t count = scoop_platform_bundle()->thread_vm->processor_count();
    if (count > 4) {
        count = 4;
    }
    const char *setting = getenv("SCOOP_GC_WORKERS");
    if (setting != NULL) {
        char *end = NULL;
        unsigned long requested = strtoul(setting, &end, 10);
        if (setting == end || *end != '\0' || requested == 0 || requested > GC_MARK_MAX_WORKERS) {
            heap_fatal("SCOOP_GC_WORKERS must be an integer from 1 to 8");
        }
        count = (size_t)requested;
        scoop_gc_marker.forced = true;
    }
    for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        worker->id = index;
        if (pthread_mutex_init(&worker->queue.lock, NULL) != 0) {
            heap_fatal("failed to initialize a mark queue");
        }
    }
    scoop_gc_marker.configured_workers = count;
    scoop_gc_marker.initialized = true;
}

void scoop_gc_mark_pool_prepare(bool minor) {
    if (!scoop_gc_marker.initialized) {
        initialize();
    }
    size_t count = scoop_gc_marker.configured_workers;
    if (!scoop_gc_marker.forced && (minor || committed_bytes < ((size_t)4 << 20))) {
        count = 1;
    }
    if (count > 1 && scoop_gc_marker.created_workers == 0) {
        for (size_t index = 1; index < count; index++) {
            ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
            worker->initial_phase = scoop_gc_marker.phase;
            if (pthread_create(&worker->thread, NULL, worker_main, worker) != 0) {
                stop_threads();
                scoop_gc_marker.configured_workers = count = 1;
                scoop_gc_heap_state.metrics.worker_creation_failures++;
                break;
            }
            scoop_gc_marker.created_workers++;
        }
    }
    pthread_mutex_lock(&scoop_gc_marker.lock);
    scoop_gc_marker.active_workers = count;
    pthread_mutex_unlock(&scoop_gc_marker.lock);
}

void scoop_gc_mark_pool_run(void) {
    pthread_mutex_lock(&scoop_gc_marker.lock);
    scoop_gc_marker.finished_workers = 0;
    scoop_gc_marker.phase++;
    pthread_cond_broadcast(&scoop_gc_marker.phase_changed);
    pthread_mutex_unlock(&scoop_gc_marker.lock);
    run_worker(&scoop_gc_marker.workers[0]);
    pthread_mutex_lock(&scoop_gc_marker.lock);
    while (scoop_gc_marker.finished_workers + 1 != scoop_gc_marker.active_workers) {
        pthread_cond_wait(&scoop_gc_marker.finished, &scoop_gc_marker.lock);
    }
    pthread_mutex_unlock(&scoop_gc_marker.lock);
}

void scoop_gc_mark_shutdown(void) {
    if (!scoop_gc_marker.initialized) {
        return;
    }
    stop_threads();
    scoop_gc_mark_dispose();
    for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        free(worker->queue.tasks);
        free(worker->blocks);
        worker->queue.tasks = NULL;
        worker->blocks = NULL;
        pthread_mutex_destroy(&worker->queue.lock);
    }
    scoop_gc_marker.initialized = false;
}
