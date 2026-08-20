/* Batched local queues; pending includes queued, active and publishing work. */
#include <stdlib.h>

#include "mark_internal.h"

static void grow_queue(ScoopGcMarkQueue *queue) {
    size_t capacity = queue->capacity == 0 ? 32 : queue->capacity * 2;
    if (capacity < queue->capacity || capacity > SIZE_MAX / sizeof *queue->tasks) {
        heap_fatal("mark queue size overflow");
    }
    ScoopGcMarkTask *tasks = malloc(capacity * sizeof *tasks);
    if (tasks == NULL) {
        heap_fatal("out of memory growing a mark queue");
    }
    for (size_t index = 0; index < queue->count; index++) {
        tasks[index] = queue->tasks[(queue->first + index) % queue->capacity];
    }
    free(queue->tasks);
    queue->tasks = tasks;
    queue->capacity = capacity;
    queue->first = 0;
}

void scoop_gc_mark_queue_push(ScoopGcMarkWorker *worker, ScoopGcMarkTask task) {
    /* The parent remains outstanding until every child has been published. */
    size_t previous = atomic_fetch_add_explicit(&scoop_gc_marker.pending, 1, memory_order_relaxed);
    ScoopGcMarkQueue *queue = &worker->queue;
    pthread_mutex_lock(&queue->lock);
    if (queue->count == queue->capacity) {
        grow_queue(queue);
    }
    queue->tasks[(queue->first + queue->count) % queue->capacity] = task;
    queue->count++;
    pthread_mutex_unlock(&queue->lock);
    /* A lone successor can be consumed by its active publisher without waking the pool. */
    if (previous >= 2) {
        scoop_gc_mark_notify_work();
    }
}

static bool take(ScoopGcMarkQueue *queue, bool steal, ScoopGcMarkTask *task) {
    pthread_mutex_lock(&queue->lock);
    bool found = queue->count != 0;
    if (found) {
        size_t index = steal ? queue->first : (queue->first + queue->count - 1) % queue->capacity;
        *task = queue->tasks[index];
        if (steal) {
            queue->first = (queue->first + 1) % queue->capacity;
        }
        queue->count--;
    }
    pthread_mutex_unlock(&queue->lock);
    return found;
}

bool scoop_gc_mark_queue_take(ScoopGcMarkWorker *worker, ScoopGcMarkTask *task) {
    if (take(&worker->queue, false, task)) {
        return true;
    }
    for (size_t distance = 1; distance < scoop_gc_marker.active_workers; distance++) {
        size_t index = (worker->id + distance) % scoop_gc_marker.active_workers;
        if (take(&scoop_gc_marker.workers[index].queue, true, task)) {
            worker->stolen_tasks++;
            return true;
        }
    }
    return false;
}

void scoop_gc_mark_task_done(void) {
    size_t previous = atomic_fetch_sub_explicit(&scoop_gc_marker.pending, 1, memory_order_acq_rel);
    if (previous == 0) {
        heap_fatal("mark task completed without outstanding work");
    }
    if (previous == 1) {
        scoop_gc_mark_notify_work();
    }
}
