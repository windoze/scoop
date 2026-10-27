#include <sys/wait.h>
#include <unistd.h>

#include "parallel_mark_fixture.h"

#undef stress_move

static void check_values(ParallelFixture *fixture, ParallelNode *pinned) {
    ParallelEdge *edges = parallel_edges(fixture);
    assert(edges[0].left == pinned && pinned->shared == fixture->objects[1]);
    for (size_t index = 0; index < PARALLEL_NODE_COUNT; index++) {
        ParallelNode *node = edges[index].left;
        assert(node->value == index && node->shared == fixture->objects[1]);
        assert(edges[index].right == edges[index % 8].left);
        if (index != 0) {
            assert(node->next == edges[index - 1].left);
        }
    }
}

static void check_remembered_ranges(ParallelFixture *fixture, bool survivors) {
    scoop_gc_heap_lock();
    scoop_gc_heap_state.survivor_enabled = survivors;
    scoop_gc_heap_unlock();
    for (size_t index = 0; index < PARALLEL_NODE_COUNT; index++) {
        NurseryNode *young = nursery_node(100000 + index);
        young->next = fixture->objects[1];
        ParallelEdge *edge = &parallel_edges(fixture)[index];
        edge->right = young;
        edge->padding = index ^ UINT64_C(0x123456789abcdef0);
        scoop_rt_gc_write_barrier(&edge->right, sizeof edge->right);
    }
    /* A 24-byte element can straddle a card-group boundary. The two references
     * and non-reference word must retain their individual range semantics. */
    for (size_t round = 0; round < 2; round++) {
        nursery_collect(true);
        size_t young_count = 0;
        for (size_t index = 0; index < PARALLEL_NODE_COUNT; index++) {
            ParallelEdge *edge = &parallel_edges(fixture)[index];
            NurseryNode *current = edge->right;
            assert(current->value == 100000 + index && current->next == fixture->objects[1]);
            assert(((ParallelNode *)edge->left)->value == index);
            assert(edge->padding == (index ^ UINT64_C(0x123456789abcdef0)));
            young_count += scoop_gc_is_young_object_locked(current);
        }
        assert(survivors && round == 0 ? young_count != 0 : young_count == 0);
    }
    for (size_t index = 0; index < PARALLEL_NODE_COUNT; index++) {
        ParallelEdge *edges = parallel_edges(fixture);
        edges[index].right = edges[index % 8].left;
        scoop_rt_gc_write_barrier(&edges[index].right, sizeof edges[index].right);
    }
    scoop_gc_heap_lock();
    scoop_gc_heap_state.survivor_enabled = false;
    scoop_gc_heap_unlock();
}

static void run_case(size_t workers) {
    ParallelFixture fixture = {0};
    parallel_start(&fixture, workers);
    ParallelNode *pinned = parallel_edges(&fixture)[0].left;
    assert(scoop_rt_pin(pinned) == pinned);
    void **aliases[] = {(void **)&pinned->next, (void **)&pinned->shared};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, aliases, 2);

    /* Exercise actual relocation even when the runtime has no stress capability. */
    nursery_collect(true);
    assert(scoop_rt_gc_debug_last_moved_count() > 0);
    check_values(&fixture, pinned);

    /* These native roots alias slots also visited by live-object update. */
#if SCOOP_ENABLE_GC_STRESS
    scoop_gc_heap_state.stress_move = true;
#endif
    for (size_t round = 0; round < 3; round++) {
        parallel_check_full(workers, PARALLEL_NODE_COUNT + 4, PARALLEL_NODE_COUNT / 1024);
        check_values(&fixture, pinned);
    }
#if SCOOP_ENABLE_GC_STRESS
    scoop_gc_heap_state.stress_move = false;
#endif

    NurseryNode *young = nursery_node(7331);
    pinned->next = (ParallelNode *)young;
    scoop_rt_gc_write_barrier(&pinned->next, sizeof pinned->next);
    nursery_collect(true);
    young = (NurseryNode *)pinned->next;
    assert(young->value == 7331 && !scoop_gc_is_young_object_locked(young));
    check_values(&fixture, pinned);
    pinned->next = NULL;
    scoop_rt_gc_write_barrier(&pinned->next, sizeof pinned->next);

    check_remembered_ranges(&fixture, false);
    check_remembered_ranges(&fixture, true);
    check_values(&fixture, pinned);

    scoop_rt_pop_native_roots(&roots);
    assert(scoop_rt_unpin(pinned) == pinned);
    parallel_finish(&fixture);
}

int main(void) {
    const size_t workers[] = {1, 2, 4, 8};
    for (size_t index = 0; index < sizeof workers / sizeof *workers; index++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            run_case(workers[index]);
            _exit(0);
        }
        int status = 0;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    puts("parallel update preserves aliased native roots, pinned fields and remembered slots");
}
