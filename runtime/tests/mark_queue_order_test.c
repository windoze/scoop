#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "../src/gc/mark_internal.h"

static void push(ScoopGcMarkWorker *owner, size_t begin, size_t end) {
    for (size_t index = begin; index < end; index++) {
        scoop_gc_mark_queue_push(owner, (ScoopGcMarkTask){
            .kind = SCOOP_MARK_BATCH_TASK,
            .data.batch.scans = index + 1,
        });
    }
}

static void expect(ScoopGcMarkWorker *consumer, size_t index) {
    ScoopGcMarkTask task;
    assert(scoop_gc_mark_queue_take(consumer, &task));
    assert(task.kind == SCOOP_MARK_BATCH_TASK && task.data.batch.scans == index + 1);
    scoop_gc_mark_task_done();
}

int main(void) {
    alarm(20);
    /* A non-power-of-two worker count exercises wrapping across the worker set. */
    assert(setenv("SCOOP_GC_WORKERS", "5", 1) == 0);
    scoop_gc_mark_pool_prepare(false);
    for (size_t index = 0; index < 5; index++) {
        ScoopGcMarkWorker *owner = &scoop_gc_marker.workers[index];
        ScoopGcMarkWorker *thief = &scoop_gc_marker.workers[(index + 1) % 5];
        size_t count = 32;
        for (size_t round = 0; round < 4; round++, count *= 4) {
            push(owner, 0, count);
            for (size_t item = 0; item < count / 2; item++) {
                expect(thief, item);
            }
            /* Grow after the head has advanced, then consume both ends. */
            push(owner, count, count * 3);
            for (size_t item = count * 3; item > count * 2; item--) {
                expect(owner, item - 1);
            }
            for (size_t item = count / 2; item < count * 2; item++) {
                expect(thief, item);
            }
            ScoopGcMarkTask task;
            assert(!scoop_gc_mark_queue_take(owner, &task));
            assert(!scoop_gc_mark_queue_take(thief, &task));
            assert(atomic_load(&scoop_gc_marker.pending) == 0);
        }
    }
    scoop_gc_shutdown();
    puts("wrapped and growing queues preserve owner and thief order for every worker");
}
