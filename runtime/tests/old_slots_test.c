#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/old_slots.h"

#undef stress_move

static unsigned released;

static void release_node(void *object) {
    const NurseryNode *node = object;
    assert(node->value == 771);
    assert((node->header.gc_word & GC_RELEASE_READY_BIT) == 0);
    assert(++released == 1);
}

static const ScoopTypeDescriptor released_td = {
    .type_id = 946,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .release_hook = release_node,
};

static const uint64_t aligned_scan[] = {1, 64};
static const ScoopTypeDescriptor padded_td = {
    .type_id = 944,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 40,
                       .instance_alignment = 8},
    .object_scan = nursery_node_scan,
};
static const ScoopTypeDescriptor aligned_td = {
    .type_id = 945,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 128,
                       .instance_alignment = 16},
    .object_scan = aligned_scan,
};

static void **edge(void *object, size_t offset) { return (void **)((char *)object + offset); }
static uint64_t *value(void *object, size_t offset) {
    return (uint64_t *)((char *)object + offset + sizeof(void *));
}

static void *new_value(const ScoopTypeDescriptor *td, size_t offset, uint64_t number) {
    if (td == &nursery_node_td) {
        return nursery_node(number);
    }
    size_t size = td->instance_shape.minimum_size;
    void *object = nursery_allocate(td, size);
    memset((char *)object + sizeof(ScoopObjectHeader), 0x6d, size - sizeof(ScoopObjectHeader));
    *edge(object, offset) = NULL;
    *value(object, offset) = number;
    return object;
}

static void check_slot_rollback(size_t size) {
    ScoopGcOldSlotPlan plan = {0}, retry = {0};
    ScoopGcBlockMeta *block;
    scoop_gc_heap_lock();
    void *reserved = scoop_heap_reserve_old_slot(&plan, size, &block, NULL);
    assert(reserved != NULL && block->state != SCOOP_BLOCK_EVACUATION_TARGET);
    size_t word = ((uintptr_t)reserved - (uintptr_t)scoop_heap_block_base(block)) / sizeof(uint64_t);
    assert(!scoop_heap_bit_test(block->starts, word));
    /* An abandoned temporary header must never become a published object. */
    memset(reserved, 0xcd, sizeof(ScoopObjectHeader));
    scoop_heap_rollback_old_slots();
    assert(scoop_heap_reserve_old_slot(&retry, size, &block, NULL) == reserved);
    assert(!scoop_heap_bit_test(block->starts, word));
    scoop_heap_rollback_old_slots();
    scoop_gc_heap_unlock();
}

static void check_size(const ScoopTypeDescriptor *td, size_t offset) {
    enum { COUNT = 4096 };
    void **objects = calloc(COUNT, sizeof *objects);
    void ***slots = malloc(COUNT * sizeof *slots);
    uintptr_t *original = malloc(COUNT * sizeof *original);
    assert(objects != NULL && slots != NULL && original != NULL);
    for (size_t index = 0; index < COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, COUNT);
    size_t size = td->instance_shape.minimum_size;
    for (size_t index = 0; index < COUNT; index++) {
        objects[index] = new_value(td, offset, index);
    }
    nursery_collect(true);
    for (size_t index = 0; index < COUNT; index++) {
        original[index] = (uintptr_t)objects[index];
        assert(scoop_heap_pointer_block(objects[index])->slot_size == size);
        if (index % 2 != 0) {
            objects[index] = NULL;
        }
    }
    assert(scoop_rt_pin(objects[0]) == objects[0]);
    nursery_collect(false);
    uint64_t occupied = nursery_metrics().heap_committed_bytes;
    assert(scoop_rt_gc_stats() == COUNT / 2);
    assert(scoop_gc_heap_state.old_pressure_bytes == COUNT / 2 * size);
    check_slot_rollback(size);
    for (size_t index = 0; index < COUNT; index += 2) {
        assert((uintptr_t)objects[index] == original[index]);
        assert(*value(objects[index], offset) == index);
        objects[index + 1] = new_value(td, offset, 100000 + index);
        *edge(objects[index + 1], offset) = objects[0];
        *edge(objects[index], offset) = objects[index + 1];
        scoop_rt_gc_write_barrier(edge(objects[index], offset), sizeof(void *));
    }
    /* Dirty old ranges overlap the new occupants' blocks after reservation.
     * Their references must still be updated exactly once through the live set. */
    nursery_collect(true);
    assert(nursery_metrics().heap_committed_bytes == occupied);
    assert(scoop_rt_gc_stats() == COUNT);
    assert(scoop_gc_heap_state.old_pressure_bytes == COUNT * size);
    for (size_t index = 0; index < COUNT; index += 2) {
        void *old = objects[index], *replacement = objects[index + 1];
        assert((uintptr_t)old == original[index]);
        assert(*edge(old, offset) == replacement && *edge(replacement, offset) == objects[0]);
        assert(*value(replacement, offset) == 100000 + index);
        assert((uintptr_t)replacement % td->instance_shape.instance_alignment == 0);
        if (size > offset + 2 * sizeof(void *)) {
            assert(((unsigned char *)replacement)[size - 1] == 0x6d);
        }
    }
    void *pinned = objects[0];
    scoop_gc_heap_state.stress_move = true;
    nursery_collect(false);
    scoop_gc_heap_state.stress_move = false;
    assert(objects[0] == pinned);
    for (size_t index = 0; index < COUNT; index += 2) {
        assert(*edge(objects[index], offset) == objects[index + 1]);
        assert(*edge(objects[index + 1], offset) == objects[0]);
        assert(*value(objects[index], offset) == index);
        assert(*value(objects[index + 1], offset) == 100000 + index);
    }
    assert(scoop_rt_unpin(objects[0]) == objects[0]);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    assert(scoop_gc_heap_state.old_pressure_bytes == 0);
    free(original);
    free(slots);
    free(objects);
}

static void check_pretenured_release(void) {
    void *objects[3] = {0};
    void **slots[] = {&objects[0], &objects[1], &objects[2]};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 3);
    objects[0] = nursery_node(770);
    for (size_t index = 1; index < 3; index++) {
        NurseryNode *node = nursery_allocate(&released_td, sizeof *node);
        node->value = 770 + index;
        if (index == 1) {
            __atomic_fetch_or(&node->header.gc_word, GC_RELEASE_READY_BIT, __ATOMIC_RELEASE);
        }
        objects[index] = node;
    }
    nursery_collect(true);
    assert(scoop_heap_pointer_block(objects[0])->slot_size == sizeof(NurseryNode));
    assert(scoop_heap_pointer_block(objects[1])->slot_size == 0);
    assert(scoop_heap_pointer_block(objects[2])->slot_size == 0);
    assert(released == 0);
    objects[1] = objects[2] = NULL;
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 1 && released == 1);
    assert(((NurseryNode *)objects[0])->value == 770);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0 && released == 1);
}

static void run_case(size_t workers) {
    char count[2] = {(char)('0' + workers), '\0'};
    assert(setenv("SCOOP_GC_OLD_SLOTS", "1", 1) == 0);
    assert(setenv("SCOOP_GC_WORKERS", count, 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &padded_td, &aligned_td, &released_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 4, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    check_size(&nursery_node_td, 16);
    check_size(&padded_td, 16);
    check_size(&aligned_td, 64);
    check_pretenured_release();
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
}

int main(void) {
    for (size_t workers = 1; workers <= 4; workers += 3) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            run_case(workers);
            _exit(0);
        }
        int status = 0;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    puts("old slots reuse dead cells and preserve roots, padding, alignment and pins");
}
