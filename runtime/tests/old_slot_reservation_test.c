#include "nursery_fixture.h"
#include "../src/gc/old_slots.h"

static ScoopTypeDescriptor descriptors[GC_OLD_SLOT_CLASSES];

static int compare_address(const void *left, const void *right) {
    uintptr_t a = *(const uintptr_t *)left;
    uintptr_t b = *(const uintptr_t *)right;
    return (a > b) - (a < b);
}

static void check_reservations(size_t size) {
    uint64_t committed = nursery_metrics().heap_committed_bytes;
    scoop_gc_heap_lock();
    size_t index = size / sizeof(uint64_t) - 2;
    size_t available = scoop_gc_heap_state.old_slots_free[index];
    assert(available > 64);
    uintptr_t *expected = malloc(available * sizeof *expected);
    uintptr_t *reserved = malloc(available * sizeof *reserved);
    assert(expected != NULL && reserved != NULL);
    size_t found = 0;
    for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        if (region->large) {
            continue;
        }
        for (size_t block_index = 0; block_index < GC_REGION_BLOCKS; block_index++) {
            ScoopGcBlockMeta *block = &region->blocks[block_index];
            if (!active_head(block) || block->generation != SCOOP_GC_OLD ||
                block->slot_size != size) {
                continue;
            }
            /* Independently enumerate complete cells from the published layout. */
            for (size_t offset = GC_LINE_SIZE; offset + size <= GC_BLOCK_SIZE; offset += size) {
                if (!bit_test(block->starts, offset / sizeof(uint64_t))) {
                    assert(found < available);
                    expected[found++] = (uintptr_t)block_base(block) + offset;
                }
            }
        }
    }
    assert(found == available);
    qsort(expected, available, sizeof *expected, compare_address);
    for (size_t round = 0; round < 3; round++) {
        ScoopGcOldSlotPlan plan = {0};
        size_t take = round == 0 ? available / 2 + 1 : available;
        for (size_t cell = 0; cell < take; cell++) {
            ScoopGcBlockMeta *block;
            void *object = scoop_heap_reserve_old_slot(&plan, size, &block, NULL);
            assert(object != NULL && block->state != SCOOP_BLOCK_EVACUATION_TARGET);
            size_t offset = (uintptr_t)object - (uintptr_t)block_base(block);
            assert(offset >= GC_LINE_SIZE && offset + size <= GC_BLOCK_SIZE);
            assert((offset - GC_LINE_SIZE) % size == 0);
            assert(!bit_test(block->starts, offset / sizeof(uint64_t)));
            reserved[cell] = (uintptr_t)object;
            memset(object, 0xcd, sizeof(ScoopObjectHeader));
        }
        qsort(reserved, take, sizeof *reserved, compare_address);
        for (size_t cell = 0; cell < take; cell++) {
            assert(cell == 0 || reserved[cell - 1] != reserved[cell]);
            assert(bsearch(&reserved[cell], expected, available, sizeof *expected,
                           compare_address) != NULL);
        }
        if (take == available) {
            assert(memcmp(reserved, expected, take * sizeof *reserved) == 0);
        }
        /* Both partial and exhausted words must restart from unchanged starts. */
        scoop_heap_rollback_old_slots();
        assert(scoop_gc_heap_state.old_slots_free[index] == available);
    }
    free(reserved);
    free(expected);
    scoop_gc_heap_unlock();
    assert(nursery_metrics().heap_committed_bytes == committed);
}

static void check_size(const ScoopTypeDescriptor *td) {
    size_t size = td->instance_shape.minimum_size;
    size_t count = 2 * (GC_REGULAR_MAX / size) + 17;
    void **objects = calloc(count, sizeof *objects);
    void ***slots = malloc(count * sizeof *slots);
    assert(objects != NULL && slots != NULL);
    for (size_t index = 0; index < count; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, count);
    for (size_t index = 0; index < count; index++) {
        objects[index] = nursery_allocate(td, size);
        memset((char *)objects[index] + sizeof(ScoopObjectHeader), (int)(index & 255),
               size - sizeof(ScoopObjectHeader));
    }
    nursery_collect(true);
    size_t kept = 0;
    for (size_t index = 0; index < count; index++) {
        if (index % 128 >= 80 && index % 11 != 0) {
            objects[index] = NULL;
        } else {
            kept++;
        }
    }
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == kept + 1);
    check_reservations(size);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == kept + 1);
    for (size_t index = 0; index < count; index++) {
        if (objects[index] == NULL) {
            continue;
        }
        assert(((ScoopObjectHeader *)objects[index])->td == td);
        for (size_t byte = sizeof(ScoopObjectHeader); byte < size; byte++) {
            assert(((unsigned char *)objects[index])[byte] == (index & 255));
        }
    }
    scoop_rt_pop_native_roots(&roots);
    free(slots);
    free(objects);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 1);
}

int main(void) {
    assert(setenv("SCOOP_GC_OLD_SLOTS", "2", 1) == 0);
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
    const ScoopTypeDescriptor *types[GC_OLD_SLOT_CLASSES + 1];
    for (size_t index = 0; index < GC_OLD_SLOT_CLASSES; index++) {
        descriptors[index] = (ScoopTypeDescriptor){
            .type_id = 1100 + index,
            .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                               .minimum_size = (index + 2) * sizeof(uint64_t),
                               .instance_alignment = 8},
        };
        types[index] = &descriptors[index];
    }
    types[GC_OLD_SLOT_CLASSES] = &nursery_node_td;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, GC_OLD_SLOT_CLASSES + 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *anchor = nursery_node(771);
    void **anchor_slot = &anchor;
    ScoopNativeRootFrame anchor_roots;
    scoop_rt_push_native_roots(&anchor_roots, &anchor_slot, 1);
    for (size_t index = 0; index < GC_OLD_SLOT_CLASSES; index++) {
        check_size(types[index]);
        assert(((NurseryNode *)anchor)->value == 771);
    }
    scoop_rt_pop_native_roots(&anchor_roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("all exact slot classes reserve each complete hole once and recover after rollback");
}
