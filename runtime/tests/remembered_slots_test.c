#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mark_internal.h"
#include "../src/gc_binding/object.h"

#undef malloc
extern void *malloc(size_t size);

#define ELEMENTS 8192

typedef struct Pair {
    NurseryNode *left;
    uintptr_t padding;
    NurseryNode *right;
} Pair;

static const uint64_t pair_scan[] = {2, offsetof(Pair, left), offsetof(Pair, right)};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, sizeof(Pair),
                                     (uintptr_t)pair_scan};
static const ScoopTypeDescriptor array_td = {
    .type_id = 1301,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = sizeof(Pair),
                       .inline_stride = sizeof(Pair),
                       .inline_alignment = 8,
                       .inline_scan = pair_scan},
    .object_scan = array_scan,
};

static _Atomic unsigned groups_until_failure;
static _Atomic bool failed;
static _Atomic unsigned seen[ELEMENTS][2];
static _Atomic uintptr_t group_writers[8];
static Pair *pairs;
static void **old_slot;
static unsigned old_slot_seen;

void *scoop_test_remembered_malloc(size_t size) {
    if (size == sizeof(ScoopGcRememberedGroup)) {
        unsigned remaining = atomic_load(&groups_until_failure);
        while (remaining != 0 &&
               !atomic_compare_exchange_weak(&groups_until_failure, &remaining, remaining - 1)) {
        }
        if (remaining == 1) {
            atomic_store(&failed, true);
            return NULL;
        }
    }
    return malloc(size);
}

static void forward_slot(void **slot, void *context) {
    (void)context;
    uintptr_t address = (uintptr_t)slot;
    uintptr_t first = (uintptr_t)pairs;
    if (address >= first && address < first + ELEMENTS * sizeof(Pair)) {
        size_t index = (address - first) / sizeof(Pair);
        size_t offset = (address - first) % sizeof(Pair);
        assert(offset == offsetof(Pair, left) || offset == offsetof(Pair, right));
        atomic_fetch_add(&seen[index][offset == offsetof(Pair, right)], 1);
        ScoopGcRegion *region = scoop_heap_region_for_address(first);
        size_t group = (address - region->base) / (GC_CARDS_PER_GROUP << GC_CARD_SHIFT);
        assert(group < sizeof group_writers / sizeof *group_writers);
        uintptr_t thread = (uintptr_t)pthread_self();
        uintptr_t previous = 0;
        if (!atomic_compare_exchange_strong(&group_writers[group], &previous, thread)) {
            assert(previous == thread);
        }
    } else if (slot == old_slot) {
        old_slot_seen++;
    }
    if (*slot != NULL) {
        void *forwarded = scoop_heap_forwarding_address(*slot);
        if (forwarded != NULL) {
            *slot = forwarded;
        }
    }
}

static void forward_entry(void **entry, void *context) {
    scoop_gc_scan_descriptor(*entry, scoop_gc_object_scan(*entry), forward_slot, context,
                             0, UINTPTR_MAX);
}

static void run_case(size_t workers, unsigned mode) {
    char setting[8];
    snprintf(setting, sizeof setting, "%zu", workers);
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    assert(setenv("SCOOP_GC_OLD_SLOTS", "0", 1) == 0);
    assert(setenv("SCOOP_GC_PROMOTE_IN_PLACE", "0", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&array_td, &nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());

    void *roots[5] = {0};
    void **root_slots[5];
    for (size_t index = 0; index < 5; index++) {
        root_slots[index] = &roots[index];
    }
    ScoopNativeRootFrame native;
    scoop_rt_push_native_roots(&native, root_slots, 5);
    roots[0] = nursery_allocate(&array_td, 24 + ELEMENTS * sizeof(Pair));
    ((ScoopArray *)roots[0])->size = ELEMENTS;
    roots[1] = nursery_node(1);
    roots[2] = nursery_node(2);
    nursery_collect(false);
    assert(nursery_metrics().full_collections != 0);
    assert(!scoop_gc_is_young_object_locked(roots[1]));
    roots[3] = nursery_node(77);
    /* Minor retains a pinned block; place the movable target in a different block. */
    for (size_t index = 0; index < GC_BLOCK_SIZE / sizeof(NurseryNode); index++) {
        (void)nursery_node(index);
    }
    roots[4] = nursery_node(88);
    assert(pointer_block(roots[3]) != pointer_block(roots[4]));
    void *original = roots[3];
    void *pinned = roots[4];
    assert(scoop_rt_pin(pinned) == pinned);
    pairs = (Pair *)((char *)roots[0] + 24);
    size_t expected_slots = 1;
    for (size_t index = 0; index < ELEMENTS; index++) {
        pairs[index].left = index % 4 == 0 ? roots[3] : index % 4 == 1 ? roots[2]
                                                : index % 4 == 2 ? NULL : roots[4];
        pairs[index].right = index % 3 == 0 ? roots[3] : NULL;
        pairs[index].padding = 0xabcdef;
        expected_slots += (index % 4 == 0 || index % 4 == 3) + (index % 3 == 0);
    }
    old_slot = (void **)&((NurseryNode *)roots[1])->next;
    *old_slot = roots[3];
    scoop_rt_gc_write_barrier(pairs, ELEMENTS * sizeof(Pair));
    scoop_rt_gc_write_barrier(old_slot, sizeof *old_slot);

    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    assert(scoop_gc_stop_world());
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    scoop_gc_set_pin_frames_locked(true);
    scoop_gc_retire_allocations();
    size_t blocks = scoop_gc_heap_begin_collection_locked(true);
    scoop_gc_mark_begin(true, blocks);
    scoop_gc_mark_roots();
    if (mode == 2) {
        assert(scoop_gc_mark_dirty_groups(true, true));
    } else {
        if (mode == 1) {
            groups_until_failure = 2;
        }
        scoop_gc_mark_remembered();
    }
    assert(scoop_gc_mark_finish(true) == 2);
    if (mode == 2) {
        scoop_gc_mark_restore_remembered();
    }
    bool complete = true;
    size_t slots = 0;
    size_t groups = 0;
    for (size_t index = 0; index < workers; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        complete &= worker->remembered_complete;
        for (ScoopGcRememberedGroup *group = worker->remembered_slots; group != NULL;
             group = group->next) {
            for (size_t word = 0; word < GC_REMEMBERED_BITMAP_WORDS; word++) {
                slots += (size_t)__builtin_popcountll(group->slots[word]);
            }
            groups++;
        }
    }
    assert(complete == (mode == 0));
    assert(atomic_load(&failed) == (mode == 1));
    if (mode == 0) {
        assert(slots == expected_slots && groups > workers);
    } else if (mode == 2) {
        assert(slots == 0 && groups == 0);
    }
    ScoopGcSurvivors survivors;
    assert(scoop_gc_heap_plan_moving_locked(true, &survivors));
    assert(moved_objects == 1 && survivors.objects == 0);
    for (size_t index = 0; index < 5; index++) {
        forward_slot(&roots[index], NULL);
    }
    scoop_gc_update_live(true, forward_slot, forward_entry, NULL);
    assert(roots[3] != original && roots[4] == pinned);
    assert(((NurseryNode *)roots[3])->value == 77 && ((NurseryNode *)roots[4])->value == 88);
    assert(*old_slot == roots[3] && old_slot_seen == 1);
    for (size_t index = 0; index < ELEMENTS; index++) {
        void *left = index % 4 == 0 ? roots[3] : index % 4 == 1 ? roots[2]
                                             : index % 4 == 2 ? NULL : roots[4];
        assert(pairs[index].left == left);
        assert(pairs[index].right == (index % 3 == 0 ? roots[3] : NULL));
        assert(pairs[index].padding == 0xabcdef);
        assert(atomic_load(&seen[index][0]) == (mode != 0 || index % 4 == 0 || index % 4 == 3));
        assert(atomic_load(&seen[index][1]) == (mode != 0 || index % 3 == 0));
    }
    scoop_gc_mark_dispose();
    for (size_t index = 0; index < workers; index++) {
        assert(scoop_gc_marker.workers[index].remembered_slots == NULL);
        assert(!scoop_gc_marker.workers[index].remembered_complete);
    }
    /* Inspect actual relocated slots at the update boundary; other tests complete reclaim. */
    _exit(0);
}

int main(void) {
    alarm(30);
    for (size_t workers = 1; workers <= 4; workers *= 4) {
        for (unsigned mode = 0; mode < 3; mode++) {
            pid_t child = fork();
            assert(child >= 0);
            if (child == 0) {
                run_case(workers, mode);
            }
            int status = 0;
            assert(waitpid(child, &status, 0) == child);
            assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
        }
    }
    puts("remembered slots preserve relocation, group ownership and incomplete-record fallback");
}
