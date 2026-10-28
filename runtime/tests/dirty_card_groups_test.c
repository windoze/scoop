#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <string.h>

#include "nursery_fixture.h"
#include "../src/gc_binding/scan.h"

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

typedef struct RangeCoverage {
    ScoopArray *object;
    uintptr_t group_base;
    uint64_t cards;
    size_t slots;
    size_t ranges;
    bool seen[ELEMENT_COUNT];
} RangeCoverage;

static void record_slot(void **slot, void *context) {
    RangeCoverage *coverage = context;
    uintptr_t payload = (uintptr_t)coverage->object->elements;
    uintptr_t address = (uintptr_t)slot;
    assert(address >= payload && (address - payload) % sizeof(void *) == 0);
    size_t index = (address - payload) / sizeof(void *);
    assert(index < coverage->object->size && !coverage->seen[index]);
    assert(address >= coverage->group_base && address < coverage->group_base + GC_BLOCK_SIZE);
    size_t card = (address - coverage->group_base) >> GC_CARD_SHIFT;
    assert((coverage->cards & (UINT64_C(1) << card)) != 0);
    coverage->seen[index] = true;
    coverage->slots++;
}

static void record_range(void *object, uintptr_t begin, uintptr_t end, void *context) {
    RangeCoverage *coverage = context;
    if (object != coverage->object) {
        return;
    }
    assert(begin >= coverage->group_base && begin < end);
    assert(end <= coverage->group_base + GC_BLOCK_SIZE);
    for (uintptr_t address = begin; address < end; address += (size_t)1 << GC_CARD_SHIFT) {
        size_t card = (address - coverage->group_base) >> GC_CARD_SHIFT;
        assert((coverage->cards & (UINT64_C(1) << card)) != 0);
    }
    coverage->ranges++;
    scoop_gc_scan_descriptor(object, array_scan, record_slot, coverage, begin, end);
}

static void check_scan_ranges(ScoopArray *object, ScoopGcCardGroup *group) {
    ScoopGcRegion *region = group->region;
    uintptr_t base = region->base + (size_t)(group - region->card_groups) * GC_BLOCK_SIZE;
    uintptr_t begin = (uintptr_t)object->elements;
    uintptr_t end = begin + object->size * sizeof(void *);
    begin = begin < base ? base : begin;
    end = end > base + GC_BLOCK_SIZE ? base + GC_BLOCK_SIZE : end;
    size_t first = (begin - base + ((size_t)1 << GC_CARD_SHIFT) - 1) >> GC_CARD_SHIFT;
    size_t last = (end - base) >> GC_CARD_SHIFT;
    assert(last - first >= 8 && last <= GC_CARDS_PER_GROUP);
    uint64_t valid = (UINT64_MAX >> (64 - (last - first))) << first;
    uint64_t masks[] = {0, valid, valid & UINT64_C(0xaaaaaaaaaaaaaaaa),
                        (UINT64_C(7) << first) | (UINT64_C(3) << (first + 6))};
    scoop_gc_heap_lock();
    for (size_t index = 0; index < sizeof masks / sizeof *masks; index++) {
        scoop_heap_clear_dirty_card_groups(false);
        RangeCoverage coverage = {.object = object, .group_base = base, .cards = masks[index]};
        for (size_t card = 0; card < GC_CARDS_PER_GROUP; card++) {
            if ((coverage.cards & (UINT64_C(1) << card)) != 0) {
                scoop_rt_gc_write_barrier((void *)(base + (card << GC_CARD_SHIFT)), 1);
            }
        }
        assert(scoop_gc_scan_dirty_group(group, record_range, &coverage, true) ==
               (uint64_t)__builtin_popcountll(coverage.cards));
        assert(coverage.ranges ==
               (size_t)__builtin_popcountll(coverage.cards & ~(coverage.cards << 1)));
        size_t expected = 0;
        for (size_t slot = 0; slot < object->size; slot++) {
            uintptr_t address = (uintptr_t)object->elements + slot * sizeof(void *);
            bool selected = address >= base && address < base + GC_BLOCK_SIZE &&
                            (coverage.cards & (UINT64_C(1) << ((address - base) >> GC_CARD_SHIFT)));
            assert(coverage.seen[slot] == selected);
            expected += selected;
        }
        assert(coverage.slots == expected);
    }
    scoop_heap_clear_dirty_card_groups(false);
    scoop_gc_heap_unlock();
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
    /* Exercise both a spanning small object and a complete large-object group. */
    roots[ARRAY_COUNT - 1] = nursery_allocate(&array_td, 24 + 1500 * sizeof(void *));
    ((ScoopArray *)roots[ARRAY_COUNT - 1])->size = 1500;
    nursery_collect(true);
    ScoopGcBlockMeta *small = pointer_block(roots[ARRAY_COUNT - 1]);
    assert(small->head->kind == SCOOP_BLOCK_KIND_SMALL && small->head->generation == SCOOP_GC_OLD);
    check_scan_ranges(roots[ARRAY_COUNT - 1], &small->region->card_groups[small->index]);
    ScoopGcRegion *large = pointer_block(roots[0])->region;
    check_scan_ranges(roots[0], &large->card_groups[1]);
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
