#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mark_internal.h"
#include "../src/gc/mutator_internal.h"

#undef malloc
extern void *malloc(size_t size);
static bool fail_cards;
static bool cards_failed;

void *scoop_test_cards_malloc(size_t size) {
    if (fail_cards && size == sizeof(ScoopGcSavedCardChunk)) {
        fail_cards = false;
        cards_failed = true;
        return NULL;
    }
    return malloc(size);
}

#ifdef SCOOP_TEST_INCREMENTAL_PARALLEL
#define TEST_WORKERS 8
#else
#define TEST_WORKERS 1
#endif
#define CHAIN_LENGTH (131072 * TEST_WORKERS)
#define ARRAY_LENGTH (2097152 * TEST_WORKERS)

static const uint64_t element_scan[] = {1, 0};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, sizeof(void *),
                                     (uintptr_t)element_scan};
static const ScoopTypeDescriptor array_td = {
    .type_id = 970,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = sizeof(void *),
                       .inline_stride = sizeof(void *),
                       .inline_alignment = 8,
                       .inline_scan = element_scan},
    .object_scan = array_scan,
};

static void step(void) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    scoop_gc_collect_incremental_internal(true);
    scoop_thread_pop_managed_anchor(&anchor);
}

static bool marked(void *object) {
    ScoopGcBlockMeta *block = pointer_block(object);
    size_t word = ((uintptr_t)object - (uintptr_t)block_base(block)) / sizeof(uint64_t);
    return object_marked(block, word);
}

static NurseryNode **elements(void **roots) { return (NurseryNode **)((char *)roots[5] + 24); }

static void prepare_young(void **roots) {
    /* Keep the test's graph young until the deliberately scheduled first slice. */
    scoop_gc_heap_state.nursery_capacity = 64 * 1024 * 1024;
    scoop_gc_heap_state.nursery_limit = 64 * 1024 * 1024;
    collection_threshold = SIZE_MAX;
    roots[0] = NULL;
    for (uint64_t index = 0; index < CHAIN_LENGTH; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = roots[0];
        roots[0] = node;
    }
    roots[4] = nursery_node(805);
    assert(pointer_block(roots[0])->generation == SCOOP_GC_YOUNG);
}

static void check_chain(void **roots, uint64_t detached) {
    NurseryNode *node = roots[0];
    for (uint64_t count = CHAIN_LENGTH; count != detached; count--) {
        assert(node != NULL && node->value == count - 1);
        node = node->next;
    }
    assert(node == NULL);
}

static NurseryNode *detach_white_tail(void **roots, uint64_t value) {
    NurseryNode *previous = roots[0];
    while (previous->next->next != NULL) {
        previous = previous->next;
    }
    NurseryNode *tail = previous->next;
    assert(!marked(tail) && tail->value == value);
    previous->next = NULL;
    scoop_rt_gc_write_barrier(&previous->next, sizeof previous->next);
    return tail;
}

static void changed_edges(void **roots) {
    prepare_young(roots);
    scoop_gc_mutator_invalidate(scoop_gc_current_mutator());
    roots[2] = nursery_node(807);
    void *remembered_source = roots[2];
    elements(roots)[0] = roots[2];
    scoop_rt_gc_write_barrier(&elements(roots)[0], sizeof(void *));
    roots[2] = NULL;
    scoop_gc_mutator_invalidate(scoop_gc_current_mutator());
    roots[6] = nursery_node(909);
    void *temporarily_pinned = roots[6];
    ScoopGcMetrics before = nursery_metrics();
    ScoopPinFrame temporary;
    scoop_rt_push_pin_frame(&temporary, roots[6]);
    step();
    assert(scoop_gc_heap_state.incremental_active && scoop_gc_heap_state.incremental_minor);
    assert(marked(roots[4]));
    scoop_rt_pop_pin_frame(&temporary);
    /* This old card is never written again; it must survive until relocation. */
    assert(scoop_gc_marker.saved_cards != NULL);

    NurseryNode *owner = roots[1];
    owner->next = detach_white_tail(roots, 0);
    scoop_rt_gc_write_barrier(&owner->next, sizeof owner->next);
    NurseryNode *bridge = roots[4];
    bridge->next = detach_white_tail(roots, 1);
    scoop_rt_gc_write_barrier(&bridge->next, sizeof bridge->next);
    const void *pinned = scoop_rt_pin(bridge);

    scoop_gc_mutator_invalidate(scoop_gc_current_mutator());
    roots[2] = nursery_node(806);
    assert(pointer_block(roots[2])->mark_index == SIZE_MAX);
    for (size_t index = 7; index < ARRAY_LENGTH; index += 67) {
        elements(roots)[index] = roots[2];
        scoop_rt_gc_write_barrier(&elements(roots)[index], sizeof(void *));
    }
    roots[2] = NULL;
    step();
    assert(scoop_gc_heap_state.incremental_active);
    assert(atomic_load(&scoop_gc_heap_state.dirty_card_groups) == NULL);
    roots[3] = nursery_node(808);
    size_t changed = ARRAY_LENGTH;
    ScoopGcRegion *region = pointer_block(roots[5])->region;
    for (size_t index = 7; index < ARRAY_LENGTH; index += 67) {
        size_t card = ((uintptr_t)&elements(roots)[index] - region->base) >> GC_CARD_SHIFT;
        ScoopGcCardGroup *group = &region->card_groups[card >> GC_CARD_GROUP_SHIFT];
        if (atomic_load(&group->next) != NULL) {
            elements(roots)[index] = roots[3];
            scoop_rt_gc_write_barrier(&elements(roots)[index], sizeof(void *));
            changed = index;
            break;
        }
    }
    assert(changed != ARRAY_LENGTH);
    roots[3] = NULL;
    size_t slices = 0;
    while (scoop_gc_heap_state.incremental_active) {
        step();
        assert(++slices < 1024);
    }
    ScoopGcMetrics after = nursery_metrics();
    assert(after.minor_collections == before.minor_collections + 1);
    assert(after.full_collections == before.full_collections);
    assert(scoop_gc_marker.saved_cards == NULL);
    assert(((NurseryNode *)roots[1])->next->value == 0);
    assert(roots[4] == pinned && ((NurseryNode *)roots[4])->next->value == 1);
    assert(roots[6] != temporarily_pinned && ((NurseryNode *)roots[6])->value == 909);
    assert(scoop_rt_unpin(pinned) == pinned);
    assert(elements(roots)[0]->value == 807);
    assert(elements(roots)[0] != remembered_source);
    for (size_t index = 7; index < ARRAY_LENGTH; index += 67) {
        assert(elements(roots)[index]->value == (index == changed ? 808 : 806));
    }
    check_chain(roots, 2);
    nursery_collect(true);
    assert(elements(roots)[0]->value == 807);
    assert(((NurseryNode *)roots[4])->next->value == 1);
    check_chain(roots, 2);
}

static void complete_or_restart(void **roots) {
    for (size_t scenario = 0; scenario < 3; scenario++) {
        prepare_young(roots);
        ScoopGcMetrics before = nursery_metrics();
        if (scenario == 2) {
            elements(roots)[0] = roots[0];
            scoop_rt_gc_write_barrier(&elements(roots)[0], sizeof(void *));
            fail_cards = true;
        }
        step();
        if (scenario != 2) {
            assert(scoop_gc_heap_state.incremental_active);
            nursery_collect(scenario == 0);
        }
        assert(!scoop_gc_heap_state.incremental_active);
        assert(scoop_gc_marker.saved_cards == NULL);
        ScoopGcMetrics after = nursery_metrics();
        assert(after.minor_collections == before.minor_collections + (scenario == 0));
        assert(after.full_collections == before.full_collections + (scenario != 0));
        check_chain(roots, 0);
    }
    assert(cards_failed);
}

int main(void) {
    alarm(40);
    assert(setenv("SCOOP_GC_INCREMENTAL", "2", 1) == 0);
#ifdef SCOOP_TEST_INCREMENTAL_PARALLEL
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
    assert(setenv("SCOOP_GC_OLD_SLOTS", "2", 1) == 0);
#else
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
    assert(setenv("SCOOP_GC_SURVIVOR", "1", 1) == 0);
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &array_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *roots[7] = {0};
    void **slots[7];
    for (size_t index = 0; index < 7; index++) {
        slots[index] = &roots[index];
    }
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 7);
    roots[5] = nursery_allocate(&array_td, 24 + ARRAY_LENGTH * sizeof(void *));
    ((ScoopArray *)roots[5])->size = ARRAY_LENGTH;
    roots[1] = nursery_node(801);
    nursery_collect(false);
    changed_edges(roots);
    complete_or_restart(roots);
    for (size_t index = 0; index < 7; index++) {
        roots[index] = NULL;
    }
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_rt_pop_native_roots(&frame);
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("minor slices preserve old cards, changed young edges, pins and full fallbacks");
}
