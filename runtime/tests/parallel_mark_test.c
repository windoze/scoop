#include <sys/wait.h>
#include <unistd.h>

#include "parallel_mark_fixture.h"

#undef stress_move

static void check_graph(ParallelFixture *fixture) {
    assert(fixture->objects[0] == fixture->objects[4]);
    ParallelEdge *edges = parallel_edges(fixture);
    for (size_t index = 0; index < PARALLEL_NODE_COUNT; index++) {
        ParallelNode *node = edges[index].left;
        assert(node->value == index && node->shared == fixture->objects[1]);
        assert(node->next == (index == 0 ? NULL : edges[index - 1].left));
        assert(edges[index].right == edges[index % 8].left);
        assert(node->shared->next->next == node->shared);
    }
    const uint64_t *numbers = (const uint64_t *)((const char *)fixture->objects[3] + 24);
    assert(numbers[0] == UINT64_C(0xa5a5a5a5a5a5a5a5));
    assert(numbers[4095] == numbers[0]);
}

static void run_case(size_t workers) {
    ParallelFixture fixture = {0};
    parallel_start(&fixture, workers);
    ScoopGcMetrics before = nursery_metrics();
    parallel_check_full(workers, PARALLEL_NODE_COUNT + 4, PARALLEL_NODE_COUNT / 1024);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.mark_reference_slots - before.mark_reference_slots == 4 * PARALLEL_NODE_COUNT + 2);
    assert(parallel_released == 8);
    check_graph(&fixture);

    void *pinned = parallel_edges(&fixture)[0].left;
    assert(scoop_rt_pin(pinned) == pinned);
    scoop_gc_heap_state.stress_move = true;
    parallel_check_full(workers, PARALLEL_NODE_COUNT + 4, PARALLEL_NODE_COUNT / 1024);
    scoop_gc_heap_state.stress_move = false;
    assert(parallel_edges(&fixture)[0].left == pinned);
    assert(scoop_rt_unpin(pinned) == pinned);
    check_graph(&fixture);

    for (size_t index = 0; index < 16; index++) {
        NurseryNode *young = nursery_node(900 + index);
        ParallelEdge *edges = parallel_edges(&fixture);
        edges[index * 1024].right = young;
        scoop_rt_gc_write_barrier(&edges[index * 1024].right, sizeof(void *));
    }
    before = nursery_metrics();
    nursery_collect(true);
    after = nursery_metrics();
    assert(after.last_mark_workers == workers);
    assert(after.traced_objects - before.traced_objects == 16);
    for (size_t index = 0; index < 16; index++) {
        NurseryNode *young = parallel_edges(&fixture)[index * 1024].right;
        assert(young->value == 900 + index && !scoop_gc_is_young_object_locked(young));
    }
    parallel_check_full(workers, PARALLEL_NODE_COUNT + 20, PARALLEL_NODE_COUNT / 1024);

    /* A single chain repeatedly leaves every queue empty while one task is active. */
    void *chain = parallel_edges(&fixture)[4095].left;
    memset(fixture.objects, 0, sizeof fixture.objects);
    fixture.objects[0] = chain;
    parallel_check_full(workers, 4098, 0);
    ParallelNode *node = fixture.objects[0];
    for (size_t remaining = 4096; remaining != 0; remaining--) {
        assert(node->value == remaining - 1);
        node = node->next;
    }
    assert(node == NULL);
    parallel_finish(&fixture);
}

int main(void) {
    const size_t counts[] = {1, 2, 4, 8};
    for (size_t index = 0; index < sizeof counts / sizeof *counts; index++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            run_case(counts[index]);
            _exit(0);
        }
        int status = 0;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    puts("1/2/4/8 markers preserve shared graphs, nested arrays, pins, minor roots and chains");
}
