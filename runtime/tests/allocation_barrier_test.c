#include <assert.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "../src/gc/gc_internal.h"
#include "../src/gc/heap_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread.h"
#include "platform/image_fixture.h"

static const size_t sizes[] = {
    56,
    64,
    72,
    80,
    128,
    136,
    512,
    520,
    2048,
    2056,
    GC_REGULAR_MAX - 16,
    GC_REGULAR_MAX,
    GC_REGULAR_MAX + 16,
};
#define SIZE_COUNT (sizeof sizes / sizeof sizes[0])

static void *allocate(const ScoopTypeDescriptor *td) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    void *object = scoop_gc_alloc_internal(td, td->instance_shape.minimum_size);
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

static void collect(void) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
}

static void check_metadata(void *object, size_t size) {
    scoop_gc_heap_lock();
    assert(scoop_gc_object_size_locked(object) == size);
    ScoopGcBlockMeta *block;
    size_t word;
    assert(scoop_heap_object_meta(object, &block, &word));
    if (size <= GC_REGULAR_MAX) {
        assert(block->kind == SCOOP_BLOCK_KIND_SMALL);
        assert((size_t)block->size_units[word] * 8 == size);
        size_t offset = (uintptr_t)object - (uintptr_t)scoop_heap_block_base(block);
        for (size_t line = offset / GC_LINE_SIZE; line <= (offset + size - 1) / GC_LINE_SIZE;
             line++) {
            assert(scoop_heap_bit_test(block->line_occupied, line));
        }
    } else {
        assert(block->kind == SCOOP_BLOCK_KIND_LARGE);
    }
    scoop_gc_heap_unlock();
}

static void *mark_shared_card(void *address) {
    for (unsigned count = 0; count < 10000; count++) {
        scoop_rt_gc_write_barrier(address, sizeof(void *));
    }
    return NULL;
}

static void check_barrier(void *object, size_t size) {
    ScoopGcRegion *region = scoop_heap_region_for_address((uintptr_t)object);
    assert(region != NULL);
    unsigned char *cards = region->cards;
    memset(cards, 0, region->size >> GC_CARD_SHIFT);
    scoop_rt_gc_write_barrier(NULL, 0);
    uintptr_t address = (uintptr_t)object + 24;
    size_t bytes = size - 24;
    scoop_rt_gc_write_barrier((void *)address, bytes);
    size_t first = (address - region->base) >> GC_CARD_SHIFT;
    size_t last = (address + bytes - 1 - region->base) >> GC_CARD_SHIFT;
    assert(first == 0 || cards[first - 1] == 0);
    assert(cards[last + 1] == 0);
    for (size_t card = first; card <= last; card++) {
        assert(cards[card] == 1);
    }
    cards[first] = 0;
    pthread_t writers[2];
    assert(pthread_create(&writers[0], NULL, mark_shared_card, (void *)address) == 0);
    assert(pthread_create(&writers[1], NULL, mark_shared_card, (void *)(address + 8)) == 0);
    assert(pthread_join(writers[0], NULL) == 0);
    assert(pthread_join(writers[1], NULL) == 0);
    assert(cards[first] == 1);
}

int main(void) {
    ScoopTypeDescriptor descriptors[SIZE_COUNT] = {0};
    const ScoopTypeDescriptor *types[SIZE_COUNT];
    for (size_t index = 0; index < SIZE_COUNT; index++) {
        descriptors[index].type_id = index + 1;
        descriptors[index].instance_shape = (ScoopTypeInstanceShapeV1){
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .minimum_size = sizes[index],
            .instance_alignment = sizes[index] % 16 == 0 ? 16 : 8,
        };
        types[index] = &descriptors[index];
    }
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, SIZE_COUNT, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *objects[SIZE_COUNT] = {0};
    void **slots[SIZE_COUNT];
    for (size_t index = 0; index < SIZE_COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, SIZE_COUNT);
    for (size_t index = 0; index < SIZE_COUNT; index++) {
        objects[index] = allocate(types[index]);
        assert((uintptr_t)objects[index] % descriptors[index].instance_shape.instance_alignment ==
               0);
        check_metadata(objects[index], sizes[index]);
        unsigned char *payload = objects[index];
        for (size_t offset = sizeof(ScoopObjectHeader); offset < sizes[index]; offset++) {
            assert(payload[offset] == 0);
        }
        payload[sizeof(ScoopObjectHeader)] = (unsigned char)(index + 1);
        payload[sizes[index] - 1] = (unsigned char)(index + 17);
    }
    // Consecutive 80-byte objects share a run and cross line boundaries.
    scoop_thread_current_required()->allocation.cursor = NULL;
    scoop_thread_current_required()->allocation.limit = NULL;
    void *first = allocate(types[3]);
    void *second = allocate(types[3]);
    assert((uintptr_t)second - (uintptr_t)first == 80);
    check_metadata(second, 80);
    collect();
    assert(scoop_rt_gc_debug_last_moved_count() > 0);
    for (size_t index = 0; index < SIZE_COUNT; index++) {
        check_metadata(objects[index], sizes[index]);
        const unsigned char *payload = objects[index];
        assert(payload[sizeof(ScoopObjectHeader)] == index + 1);
        assert(payload[sizes[index] - 1] == index + 17);
    }
    check_barrier(objects[SIZE_COUNT - 1], sizes[SIZE_COUNT - 1]);
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("regular allocation and range barrier tests passed");
}
