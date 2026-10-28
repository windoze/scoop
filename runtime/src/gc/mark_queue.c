/* Batched local queues; pending includes queued, active and publishing work. */
#include <stdlib.h>

#include "mark_internal.h"

static size_t queue_index(const ScoopGcMarkQueue *queue, size_t position) {
    /* Capacity starts at 32 and doubles; nonempty queues always have storage. */
    return position & (queue->capacity - 1);
}

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
        tasks[index] = queue->tasks[queue_index(queue, queue->first + index)];
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
    queue->tasks[queue_index(queue, queue->first + queue->count)] = task;
    queue->count++;
    pthread_mutex_unlock(&queue->lock);
    /* A lone successor can be consumed by its active publisher without waking the pool. */
    if (previous >= 2) {
        scoop_gc_mark_notify_work(false);
    }
}

/* The original links and cards remain intact through minor relocation. Workers
 * start after publication; survivor update may subsequently register new groups. */
void scoop_gc_mark_enqueue_remembered(void) {
    size_t owner = 0;
    ScoopGcCardGroup *first =
        atomic_load_explicit(&scoop_gc_heap_state.dirty_card_groups, memory_order_acquire);
    for (ScoopGcCardGroup *group = first; group != NULL;
         group = scoop_heap_next_dirty_card_group(group)) {
        ScoopGcRegion *region = group->region;
        size_t index = region->large ? 0 : (size_t)(group - region->card_groups);
        ScoopGcBlockMeta *block = &region->blocks[index];
        if (!active_head(block) || block->head->generation != SCOOP_GC_OLD) {
            continue;
        }
        scoop_gc_mark_queue_push(&scoop_gc_marker.workers[owner], (ScoopGcMarkTask){
            .kind = SCOOP_MARK_DIRTY_TASK,
            .data.dirty.group = group,
        });
        if (++owner == scoop_gc_marker.active_workers) {
            owner = 0;
        }
    }
}

static bool take(ScoopGcMarkQueue *queue, bool steal, ScoopGcMarkTask *task) {
    pthread_mutex_lock(&queue->lock);
    bool found = queue->count != 0;
    if (found) {
        size_t index = steal ? queue->first : queue_index(queue, queue->first + queue->count - 1);
        *task = queue->tasks[index];
        if (steal) {
            queue->first = queue_index(queue, queue->first + 1);
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
        size_t index = worker->id + distance;
        if (index >= scoop_gc_marker.active_workers) {
            index -= scoop_gc_marker.active_workers;
        }
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
        scoop_gc_mark_notify_work(true);
    }
}
