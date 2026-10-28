#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/gc/mark_internal.h"

static void check_slice(size_t workers, size_t expected) {
    char setting[8];
    snprintf(setting, sizeof setting, "%zu", workers);
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    scoop_gc_mark_pool_prepare(false);
    const size_t total = 16384;
    for (size_t index = 0; index < total; index++) {
        scoop_gc_mark_queue_push(&scoop_gc_marker.workers[index % workers], (ScoopGcMarkTask){0});
    }
    assert(!scoop_gc_mark_step(false));
    assert(atomic_load(&scoop_gc_marker.pending) == total - expected);
    assert(scoop_gc_heap_state.metrics.mark_tasks == expected);
    assert(atomic_load(&scoop_gc_marker.waiting_workers) == 0);
    /* Finishing the same phase drains every retained task regardless of quota. */
    assert(scoop_gc_mark_step(true));
    assert(scoop_gc_heap_state.metrics.mark_tasks == total);
    assert(atomic_load(&scoop_gc_marker.waiting_workers) == 0);
    scoop_gc_shutdown();
}

int main(void) {
    alarm(20);
    const size_t cases[][2] = {
        {1, 1024}, {2, 2048}, {4, 4096}, {5, 4095}, {8, 4096}, {12, 4092}, {16, 4096},
    };
    for (size_t index = 0; index < sizeof cases / sizeof *cases; index++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            check_slice(cases[index][0], cases[index][1]);
            _exit(0);
        }
        int status = 0;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    puts("parallel slices share their work limit and finishing drains all retained tasks");
}
