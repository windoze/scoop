/* NoGC range barrier, shared by native and generated aggregate stores. */
#include <string.h>

#include "heap_internal.h"

void scoop_runtime_record_dirty_card_group(void *mapping, size_t card_index) {
    ScoopGcRegion *region = mapping;
    ScoopGcCardGroup *group = &region->card_groups[card_index >> GC_CARD_GROUP_SHIFT];
    if (atomic_load_explicit(&group->next, memory_order_relaxed) != NULL) {
        return;
    }
    ScoopGcCardGroup *expected = NULL;
    if (!atomic_compare_exchange_strong_explicit(&group->next, &expected, group,
                                                 memory_order_relaxed, memory_order_relaxed)) {
        return;
    }
    /* The winning writer must publish before it can park. Other writers only
     * observe the claim; they never change this group's link during the cycle. */
    ScoopGcCardGroup *head =
        atomic_load_explicit(&scoop_gc_heap_state.dirty_card_groups, memory_order_relaxed);
    do {
        atomic_store_explicit(&group->next, head == NULL ? group : head, memory_order_relaxed);
    } while (!atomic_compare_exchange_weak_explicit(&scoop_gc_heap_state.dirty_card_groups, &head,
                                                    group, memory_order_release,
                                                    memory_order_relaxed));
}

void scoop_heap_clear_dirty_card_group(ScoopGcCardGroup *group) {
    size_t index = (size_t)(group - group->region->card_groups);
    memset(group->region->cards + index * GC_CARDS_PER_GROUP, 0, GC_CARDS_PER_GROUP);
    atomic_store_explicit(&group->next, NULL, memory_order_relaxed);
}

void scoop_heap_clear_dirty_card_groups(bool retain_survivors) {
    /* All writers are stopped, and no region may be retired until this ends. */
    ScoopGcCardGroup *group = atomic_exchange_explicit(&scoop_gc_heap_state.dirty_card_groups, NULL,
                                                       memory_order_acquire);
    ScoopGcCardGroup *retained = NULL;
    while (group != NULL) {
        ScoopGcCardGroup *next = scoop_heap_next_dirty_card_group(group);
        size_t index = (size_t)(group - group->region->card_groups);
        unsigned char *cards = group->region->cards + index * GC_CARDS_PER_GROUP;
        bool keep = false;
        if (retain_survivors) {
            for (size_t card = 0; card < GC_CARDS_PER_GROUP; card++) {
                cards[card] = cards[card] == 2;
                keep |= cards[card] != 0;
            }
        } else {
            memset(cards, 0, GC_CARDS_PER_GROUP);
        }
        atomic_store_explicit(&group->next, keep ? (retained == NULL ? group : retained) : NULL,
                              memory_order_relaxed);
        if (keep) {
            retained = group;
        }
        group = next;
    }
    atomic_store_explicit(&scoop_gc_heap_state.dirty_card_groups, retained, memory_order_release);
}

void scoop_rt_gc_write_barrier(const void *destination, size_t bytes) {
    uintptr_t address = (uintptr_t)destination;
    if (bytes > UINTPTR_MAX - address) {
        heap_fatal("write barrier range overflows");
    }
    while (bytes != 0) {
        ScoopGcRegion *region = scoop_heap_region_for_address(address);
        if (region == NULL) {
            heap_fatal("write barrier range lies outside heap mappings");
        }
        size_t offset = address - region->base;
        size_t part = region->size - offset;
        if (part > bytes) {
            part = bytes;
        }
        size_t first = offset >> GC_CARD_SHIFT;
        size_t last = (offset + part - 1) >> GC_CARD_SHIFT;
        for (size_t card = first; card <= last; card++) {
            /* The collector clears binary card states only while mutators are stopped. */
            __atomic_store_n(&region->cards[card], 1, __ATOMIC_RELAXED);
        }
        for (size_t group = first >> GC_CARD_GROUP_SHIFT; group <= last >> GC_CARD_GROUP_SHIFT;
             group++) {
            scoop_runtime_record_dirty_card_group(region, group << GC_CARD_GROUP_SHIFT);
        }
        address += part;
        bytes -= part;
    }
}
