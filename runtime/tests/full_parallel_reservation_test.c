#include "nursery_fixture.h"
#include <unistd.h>

#undef calloc
extern void *calloc(size_t count, size_t size);

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ready = PTHREAD_COND_INITIALIZER;
static unsigned arrivals;
static _Atomic unsigned remaining;
static _Atomic bool failed;
static bool require_overlap;

void *scoop_test_full_copy_calloc(size_t count, size_t size) {
    if (require_overlap && count == 1 && size == 6 * sizeof(uintptr_t)) {
        pthread_mutex_lock(&lock);
        arrivals++;
        pthread_cond_broadcast(&ready);
        while (arrivals < 2) {
            pthread_cond_wait(&ready, &lock);
        }
        pthread_mutex_unlock(&lock);
        unsigned current = atomic_load(&remaining);
        while (current != 0 &&
               !atomic_compare_exchange_weak(&remaining, &current, current - 1)) {
        }
        if (current == 1) {
            atomic_store(&failed, true);
            return NULL;
        }
    }
    return calloc(count, size);
}

static NurseryNode *publish_node(ScoopGcBlockMeta *block, size_t offset, uint64_t value) {
    NurseryNode *node = (void *)((char *)block_base(block) + offset);
    *node = (NurseryNode){.header = {&nursery_node_td, 0}, .value = value};
    scoop_heap_record_small_object(block, node, sizeof *node);
    return node;
}

int main(void) {
    assert(setenv("SCOOP_GC_OLD_SLOTS", "0", 1) == 0);
    alarm(30);
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    assert(nursery_node(2)->value == 2);
    nursery_collect(false);

    const size_t blocks_per_region = GC_REGION_BLOCKS - (SCOOP_ENABLE_GC_STRESS ? 1 : 0);
    const size_t target_count = blocks_per_region * GC_LINES_PER_BLOCK / 2;
    const size_t source_count = 2 * blocks_per_region * 4;
    const size_t count = target_count + source_count;
    void **objects = calloc(count, sizeof *objects);
    void ***slots = calloc(count, sizeof *slots);
    uintptr_t *original = calloc(count, sizeof *original);
    assert(objects != NULL && slots != NULL && original != NULL);
    size_t used = 0;
    scoop_gc_heap_lock();
    for (size_t index = 0; index < blocks_per_region; index++) {
        ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
        assert(block != NULL);
        /* A retained node on every other line leaves many holes in one block. */
        for (size_t line = 1; line < GC_LINES_PER_BLOCK; line += 2) {
            objects[used] = publish_node(block, line * GC_LINE_SIZE, used);
            used++;
        }
    }
    assert(used == target_count);
    for (size_t index = 0; index < 2 * blocks_per_region; index++) {
        ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
        assert(block != NULL);
        for (size_t item = 0; item < 4; item++) {
            objects[used] = publish_node(block, GC_LINE_SIZE + item * sizeof(NurseryNode), used);
            used++;
        }
    }
    scoop_gc_heap_unlock();
    assert(used == count);
    for (size_t index = 0; index < count; index++) {
        slots[index] = &objects[index];
        original[index] = (uintptr_t)objects[index];
        ((NurseryNode *)objects[index])->next = objects[(index + 317) % count];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, count);
    ScoopPinFrame pin;
    scoop_rt_push_pin_frame(&pin, objects[0]);
    uintptr_t destination = pointer_block(objects[0])->region->base;
    require_overlap = true;
    remaining = 2;
    nursery_collect(false);
    require_overlap = false;
    assert(failed && remaining == 0 && arrivals >= 2);
    assert(nursery_metrics().last_mark_workers == 4);
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    for (size_t index = 0; index < count; index++) {
        assert((uintptr_t)objects[index] == original[index]);
        assert(!pointer_block(objects[index])->copy_reserved);
    }

    /* The same graph must compact after the failed, overlapping reservations. */
    nursery_collect(false);
    assert(scoop_rt_gc_debug_last_moved_count() == source_count);
    assert(nursery_metrics().last_full_source_regions == 2);
    assert(nursery_metrics().last_full_target_regions == 1);
    for (size_t index = 0; index < count; index++) {
        NurseryNode *node = objects[index];
        assert(node->value == index && node->next == objects[(index + 317) % count]);
        assert(pointer_block(node)->region->base == destination);
        assert(((uintptr_t)node == original[index]) == (index < target_count));
        assert(!pointer_block(node)->copy_reserved);
    }
    assert(scoop_rt_gc_stats() == count);
    scoop_rt_pop_pin_frame(&pin);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    free(original);
    free(slots);
    free(objects);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("parallel full reservations own complete hole blocks and recover from partial failure");
}
