#include "no_core.h"
#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/value_shape.h"

typedef struct SharedScan {
    uint64_t sequence[4];
    uint64_t first[5];
    uint64_t second[5];
} SharedScan;

static void shared_dag(void) {
    uint64_t leaf[] = {1, 0};
    SharedScan graph[80];
    const uint64_t *child = leaf;
    for (size_t index = 0; index < 80; index++) {
        SharedScan *node = &graph[index];
        *node = (SharedScan){
            .sequence = {SCOOP_REFS_SEQUENCE, 2, (uintptr_t)node->first,
                         (uintptr_t)node->second},
            .first = {SCOOP_REFS_ARRAY, 0, 16, 32, (uintptr_t)child},
            .second = {SCOOP_REFS_ARRAY, 8, 24, 32, (uintptr_t)child},
        };
        child = node->sequence;
    }
    scoop_shape_scan_validate(child, 32);
    assert(scoop_shape_scan_equal(child, child, 0));
    uint64_t different[] = {1, 8};
    assert(!scoop_shape_scan_equal(leaf, different, 0));
}

static void large_reference_leaf(void) {
    const size_t count = 1048576;
    uint64_t *scan = malloc((count + 1) * sizeof *scan);
    uint64_t *translated = malloc((count + 1) * sizeof *translated);
    assert(scan != NULL && translated != NULL);
    scan[0] = translated[0] = count;
    for (size_t index = 0; index < count; index++) {
        scan[index + 1] = index * 8;
        translated[index + 1] = index * 8 + 16;
    }
    scoop_shape_scan_validate(scan, count * 8);
    assert(scoop_shape_scan_equal(translated, scan, 16));
    free(translated);
    free(scan);
}

static void rejects_invalid_scan(bool cycle) {
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(STDERR_FILENO);
        uint64_t scan[] = {SCOOP_REFS_ARRAY, 0, 8, 16, 0};
        scan[4] = (uintptr_t)scan;
        uint64_t outside[] = {1, 16};
        scoop_shape_scan_validate(cycle ? scan : outside, 16);
        _exit(0);
    }
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
}

int main(void) {
    scoop_test_disable_core_dumps();
    shared_dag();
    large_reference_leaf();
    rejects_invalid_scan(true);
    rejects_invalid_scan(false);
    puts("reference scan graph tests passed");
    return 0;
}
