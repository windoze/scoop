#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mutator_internal.h"

#ifdef SCOOP_TEST_INCREMENTAL_PARALLEL
#define TEST_WORKERS 4
#else
#define TEST_WORKERS 1
#endif
/* Both the chain and the dirty groups must outlast a complete marking slice. */
#define CHAIN_LENGTH (131072 * TEST_WORKERS)
#define ARRAY_LENGTH (2097152 * TEST_WORKERS)

extern bool scoop_test_vm_fail_mappings;
static size_t released;
static uint64_t detached_tail;

static void release_node(void *object) {
    assert(((NurseryNode *)object)->value == 77);
    released++;
}

static const uint64_t element_scan[] = {1, 0};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, sizeof(void *),
                                     (uintptr_t)element_scan};
static const ScoopTypeDescriptor array_td = {
    .type_id = 960,
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
static const ScoopTypeDescriptor release_td = {
    .type_id = 961,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .release_hook = release_node,
};

static void step(void) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    scoop_gc_collect_incremental_internal();
    scoop_thread_pop_managed_anchor(&anchor);
}

static bool marked(void *object) {
    ScoopGcBlockMeta *block = pointer_block(object);
    size_t word = ((uintptr_t)object - (uintptr_t)block_base(block)) / sizeof(uint64_t);
    return object_marked(block, word);
}

static ScoopArray *new_array(void) {
    ScoopArray *array = nursery_allocate(&array_td, 24 + ARRAY_LENGTH * sizeof(void *));
    array->size = ARRAY_LENGTH;
    return array;
}

static NurseryNode **elements(void **roots) { return (NurseryNode **)((char *)roots[5] + 24); }

static void prepare(void **roots) {
    roots[5] = new_array();
    for (uint64_t index = 0; index < CHAIN_LENGTH; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = roots[0];
        roots[0] = node;
    }
    roots[1] = nursery_node(801);
    nursery_collect(false);
    roots[4] = nursery_node(805);
}

static void check_chain(void **roots) {
    NurseryNode *node = roots[0];
    for (uint64_t count = CHAIN_LENGTH; count != detached_tail; count--) {
        assert(node != NULL && node->value == count - 1);
        node = node->next;
    }
    assert(node == NULL);
}

static NurseryNode *detach_white_tail(void **roots) {
    NurseryNode *previous = roots[0];
    while (previous->next->next != NULL) {
        previous = previous->next;
    }
    NurseryNode *tail = previous->next;
    assert(!marked(tail) && tail->value == detached_tail++);
    previous->next = NULL;
    scoop_rt_gc_write_barrier(&previous->next, sizeof previous->next);
    return tail;
}

static void changed_edges(void **roots) {
    prepare(roots);
    ScoopGcMutator *mutator = scoop_gc_current_mutator();
    char *cursor = mutator->allocation.cursor;
    size_t reserves = mutator->reserved_count;
    ScoopGcMetrics before = nursery_metrics();
    ScoopPinFrame temporary;
    scoop_rt_push_pin_frame(&temporary, roots[4]);
    step();
    assert(scoop_gc_heap_state.incremental_active && collection_active);
    assert(mutator->allocation.cursor == cursor && mutator->reserved_count == reserves);
    assert(nursery_metrics().full_collections == before.full_collections);
    assert(marked(roots[1]) && marked(roots[4]));
    assert((((NurseryNode *)roots[4])->header.gc_word & GC_PIN_BIT) == 0);
    scoop_rt_pop_pin_frame(&temporary);

    /* Both black old and black young owners receive otherwise unreachable white nodes. */
    NurseryNode *owner = roots[1];
    owner->next = detach_white_tail(roots);
    scoop_rt_gc_write_barrier(&owner->next, sizeof owner->next);
    NurseryNode *bridge = roots[4];
    bridge->next = detach_white_tail(roots);
    scoop_rt_gc_write_barrier(&bridge->next, sizeof bridge->next);
    const void *pinned = scoop_rt_pin(owner);

    /* Force new nursery metadata after the initial block-index assignment. */
    scoop_gc_mutator_invalidate(mutator);
    roots[2] = nursery_node(806);
    ((NurseryNode *)roots[2])->next = detach_white_tail(roots);
    scoop_rt_gc_write_barrier(&((NurseryNode *)roots[2])->next, sizeof(void *));
    assert(scoop_gc_heap_state.incremental_active);
    assert(pointer_block(roots[2])->mark_index == SIZE_MAX);
    for (size_t index = 7; index < ARRAY_LENGTH; index += 67) {
        NurseryNode **slots = elements(roots);
        slots[index] = roots[2];
        scoop_rt_gc_write_barrier(&slots[index], sizeof slots[index]);
    }
    roots[2] = NULL;
    step();
    assert(scoop_gc_heap_state.incremental_active);
    assert(atomic_load(&scoop_gc_heap_state.dirty_card_groups) == NULL);
    /* More than one slice of card groups remains queued. A later write must be read
     * by its existing task, and must not lose the group's registration. */
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
    assert(atomic_load(&scoop_gc_heap_state.dirty_card_groups) == NULL);
    roots[3] = NULL;
    size_t slices = 0;
    while (scoop_gc_heap_state.incremental_active) {
        step();
        assert(++slices < 1024);
    }
    assert(!collection_active && scoop_gc_mutator_is_retired(mutator));
    assert(nursery_metrics().full_collections == before.full_collections + 1);
    check_chain(roots);
    owner = roots[1];
    assert(owner == pinned && owner->value == 801 && owner->next->value == 0);
    bridge = roots[4];
    assert(bridge->value == 805 && bridge->next->value == 1);
    for (size_t index = 7; index < ARRAY_LENGTH; index += 67) {
        assert(elements(roots)[index]->value == (index == changed ? 808 : 806));
        if (index != changed) {
            assert(elements(roots)[index]->next->value == 2);
        }
    }
    assert(scoop_rt_unpin(pinned) == pinned);
    assert(scoop_rt_gc_stats() == CHAIN_LENGTH + 5);
}

static void complete_or_restart(void **roots) {
    ScoopGcMetrics before = nursery_metrics();
    step();
    assert(scoop_gc_heap_state.incremental_active);
    /* A nursery-full request completes the current full instead of starting minor. */
    nursery_collect(true);
    assert(!scoop_gc_heap_state.incremental_active);
    assert(nursery_metrics().minor_collections == before.minor_collections);
    assert(nursery_metrics().full_collections == before.full_collections + 1);
    check_chain(roots);

    step();
    assert(scoop_gc_heap_state.incremental_active);
    for (size_t index = 7; index < ARRAY_LENGTH; index += 67) {
        elements(roots)[index] = roots[4];
        scoop_rt_gc_write_barrier(&elements(roots)[index], sizeof(void *));
    }
    step();
    assert(scoop_gc_heap_state.incremental_active);
    nursery_collect(false);
    assert(!scoop_gc_heap_state.incremental_active);
    check_chain(roots);
    /* Cancellation releases queued group claims so an ordinary minor sees later writes. */
    roots[2] = nursery_node(809);
    elements(roots)[7] = roots[2];
    scoop_rt_gc_write_barrier(&elements(roots)[7], sizeof(void *));
    roots[2] = NULL;
    nursery_collect(true);
    assert(elements(roots)[7]->value == 809);

    step();
    assert(scoop_gc_heap_state.incremental_active);
    NurseryNode *fixed = nursery_allocate(&release_td, sizeof *fixed);
    fixed->value = 77;
    fixed->header.gc_word |= GC_RELEASE_READY_BIT;
    assert(!scoop_gc_heap_state.incremental_active && released == 0);
    /* Ready dead objects are released only when a complete cycle reclaims them. */
    step();
    assert(scoop_gc_heap_state.incremental_active && released == 0);
    roots[3] = new_array();
    assert(!scoop_gc_heap_state.incremental_active && released == 1);
    assert(((ScoopArray *)roots[3])->size == ARRAY_LENGTH);
    check_chain(roots);

    scoop_gc_mutator_invalidate(scoop_gc_current_mutator());
    scoop_gc_heap_lock();
    while (activate_small_block(SCOOP_BLOCK_MUTATOR, false) != NULL) {
        /* Leave no existing free block for the next refill. */
    }
    scoop_gc_heap_unlock();
    step();
    assert(scoop_gc_heap_state.incremental_active);
    scoop_test_vm_fail_mappings = true;
    roots[2] = nursery_node(807);
    scoop_test_vm_fail_mappings = false;
    assert(!scoop_gc_heap_state.incremental_active);
    assert(((NurseryNode *)roots[2])->value == 807);
    check_chain(roots);
}

int main(void) {
    alarm(30);
    assert(setenv("SCOOP_GC_INCREMENTAL", "1", 1) == 0);
#ifdef SCOOP_TEST_INCREMENTAL_PARALLEL
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
#else
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &array_td, &release_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *roots[6] = {0};
    void **slots[6];
    for (size_t index = 0; index < 6; index++) {
        slots[index] = &roots[index];
    }
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 6);
    changed_edges(roots);
    complete_or_restart(roots);
    for (size_t index = 0; index < 6; index++) {
        roots[index] = NULL;
    }
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0 && released == 1);
    scoop_rt_pop_native_roots(&frame);
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("incremental marking preserves changed edges, pins, roots and complete fallbacks");
    return 0;
}
