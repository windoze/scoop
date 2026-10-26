#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>

#include "nursery_fixture.h"

#define ARRAY_COUNT 8
#define ELEMENT_COUNT 12288
#define WRITERS 8

static const uint64_t element_scan[] = {1, 0};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, 8, (uintptr_t)element_scan};
static const ScoopTypeDescriptor array_td = {
    .type_id = 932,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 8,
                       .inline_stride = 8,
                       .inline_alignment = 8,
                       .inline_scan = element_scan},
    .object_scan = array_scan};

static _Atomic(bool) start;

static void *mark(void *slot) {
    while (!atomic_load_explicit(&start, memory_order_acquire)) {
        sched_yield();
    }
    for (size_t index = 0; index < 10000; index++) {
        scoop_rt_gc_write_barrier(slot, sizeof(void *));
    }
    return NULL;
}

static size_t group_count(void) {
    ScoopGcCardGroup *seen[WRITERS + 2];
    size_t count = 0;
    ScoopGcCardGroup *group =
        atomic_load_explicit(&scoop_gc_heap_state.dirty_card_groups, memory_order_acquire);
    while (group != NULL) {
        assert(count < sizeof seen / sizeof seen[0]);
        for (size_t index = 0; index < count; index++) {
            assert(seen[index] != group);
        }
        seen[count++] = group;
        group = scoop_heap_next_dirty_card_group(group);
    }
    return count;
}

static ScoopArray *array(void) {
    ScoopArray *result = nursery_allocate(&array_td, 24 + ELEMENT_COUNT * sizeof(void *));
    result->size = ELEMENT_COUNT;
    return result;
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &array_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *roots[ARRAY_COUNT] = {0};
    void **slots[ARRAY_COUNT];
    for (size_t index = 0; index < ARRAY_COUNT; index++) {
        slots[index] = &roots[index];
    }
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, ARRAY_COUNT);
    for (size_t index = 0; index < ARRAY_COUNT; index++) {
        roots[index] = array();
    }
    assert(group_count() == 0);
    for (uint64_t round = 0; round < 4; round++) {
        NurseryNode **elements = (NurseryNode **)((ScoopArray *)roots[0])->elements;
        size_t crossing = (GC_BLOCK_SIZE - GC_LINE_SIZE - 24) / sizeof(void *) - 1;
        elements[crossing] = nursery_node(51 + round);
        elements[crossing + 1] = nursery_node(71 + round);
        scoop_rt_gc_write_barrier(&elements[crossing], 2 * sizeof(void *));
        assert(group_count() == 2);
        for (size_t index = 0; index < WRITERS / 2; index++) {
            NurseryNode **owner = (NurseryNode **)((ScoopArray *)roots[index])->elements;
            owner[0] = nursery_node(91 + 20 * index + round);
        }
        NurseryNode **other = (NurseryNode **)((ScoopArray *)roots[1])->elements;
        pthread_t writers[WRITERS];
        atomic_store_explicit(&start, false, memory_order_relaxed);
        for (size_t index = 0; index < WRITERS; index++) {
            NurseryNode **owner =
                (NurseryNode **)((ScoopArray *)roots[index % (WRITERS / 2)])->elements;
            assert(pthread_create(&writers[index], NULL, mark, &owner[0]) == 0);
        }
        atomic_store_explicit(&start, true, memory_order_release);
        for (size_t index = 0; index < WRITERS; index++) {
            assert(pthread_join(writers[index], NULL) == 0);
        }
        assert(group_count() == 5);
        NurseryNode *previous = other[0];
        ScoopGcMetrics before = nursery_metrics();
        nursery_collect(true);
        ScoopGcMetrics after = nursery_metrics();
        assert(after.dirty_cards - before.dirty_cards == 6);
        assert(other[0] != previous);
        assert(elements[crossing]->value == 51 + round);
        assert(elements[crossing + 1]->value == 71 + round);
        for (size_t index = 0; index < WRITERS / 2; index++) {
            NurseryNode **owner = (NurseryNode **)((ScoopArray *)roots[index])->elements;
            assert(owner[0]->value == 91 + 20 * index + round);
        }
        assert(group_count() == 0);
    }
    /* Full collection must clear pending links before freeing their mappings. */
    NurseryNode **elements = (NurseryNode **)((ScoopArray *)roots[1])->elements;
    elements[0] = nursery_node(200);
    scoop_rt_gc_write_barrier(&elements[0], sizeof(void *));
    assert(group_count() == 1);
    for (size_t index = 0; index < ARRAY_COUNT; index++) {
        roots[index] = NULL;
    }
    nursery_collect(false);
    assert(group_count() == 0);
    roots[0] = array();
    elements = (NurseryNode **)((ScoopArray *)roots[0])->elements;
    elements[0] = nursery_node(300);
    scoop_rt_gc_write_barrier(&elements[0], sizeof(void *));
    nursery_collect(true);
    assert(elements[0]->value == 300 && group_count() == 0);
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("dirty card groups preserve sparse, crossing and concurrent writes through collection");
    return 0;
}
