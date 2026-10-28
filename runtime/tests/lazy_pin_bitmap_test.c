#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"

#undef calloc
void *calloc(size_t count, size_t size);
static size_t bitmap_allocations;
static bool fail_bitmap;

void *scoop_test_pin_bitmap_calloc(size_t count, size_t size) {
    if (count == GC_BITMAP_WORDS && size == sizeof(uint64_t)) {
        bitmap_allocations++;
        if (fail_bitmap) {
            return NULL;
        }
    }
    return calloc(count, size);
}

static void run_case(bool fail) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *objects[2] = {0};
    void **slots[] = {&objects[0], &objects[1]};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 2);
    objects[0] = nursery_node(91);
    objects[1] = nursery_node(73);
    ScoopGcBlockMeta *block = scoop_heap_pointer_block(objects[0]);
    assert(scoop_heap_pointer_block(objects[1]) == block);
    assert(block->pins == NULL && !scoop_heap_block_has_pins(block));
    scoop_gc_heap_lock();
    assert(!scoop_gc_update_pin_locked(objects[0], false));
    assert(block->pins == NULL);
    scoop_gc_heap_unlock();

    size_t before = bitmap_allocations;
    fail_bitmap = fail;
    assert(scoop_rt_pin(objects[0]) == objects[0]);
    assert(!fail);
    uint64_t *bitmap = block->pins;
    assert(bitmap != NULL && bitmap_allocations == before + 1);
    assert(scoop_rt_pin(objects[0]) == objects[0]);
    assert(scoop_rt_pin(objects[1]) == objects[1]);
    assert(block->pins == bitmap && bitmap_allocations == before + 1);
    void *first = objects[0], *second = objects[1];
    nursery_collect(true);
    assert(objects[0] == first && objects[1] == second);
    assert(nursery_metrics().minor_collections != 0);
    assert(scoop_rt_unpin(objects[0]) == first);
    assert(scoop_heap_block_has_pins(block));
    assert(scoop_rt_unpin(objects[0]) == first);
    assert(scoop_rt_unpin(objects[1]) == second);
    assert(!scoop_heap_block_has_pins(block) && block->pins == bitmap);

    /* Temporary pins reuse the buffer and clear their bits before resuming. */
    before = bitmap_allocations;
    ScoopPinFrame frame;
    scoop_rt_push_pin_frame(&frame, objects[0]);
    nursery_collect(false);
    assert(objects[0] == first && frame.object == first);
    assert(((NurseryNode *)objects[0])->value == 91);
    assert(((NurseryNode *)objects[1])->value == 73);
    assert(block->pins == bitmap && !scoop_heap_block_has_pins(block));
    assert(bitmap_allocations == before);
    scoop_rt_pop_pin_frame(&frame);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
}

int main(void) {
    alarm(30);
    for (unsigned mode = 0; mode < 2; mode++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            const struct rlimit no_core = {0, 0};
            assert(setrlimit(RLIMIT_CORE, &no_core) == 0);
            run_case(mode != 0);
            _exit(0);
        }
        int status = 0;
        assert(waitpid(child, &status, 0) == child);
        if (mode == 0) {
            assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
        } else {
            assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
        }
    }
    puts("pin bitmaps are lazy, shared by block, reusable and fail before publication");
}
