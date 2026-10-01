#include <assert.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread.h"

typedef struct TestLeaf {
    ScoopObjectHeader header;
    uint64_t value;
} TestLeaf;

static const uint64_t one_root_scan[] = {1, 0};

static const ScoopTypeDescriptor leaf_td = {
    .type_id = 101,
    .instance_shape = {
        .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
        .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
        .minimum_size = sizeof(TestLeaf),
        .instance_alignment = _Alignof(TestLeaf),
    },
    .object_scan = NULL,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .diagnostic_name = {(const uint8_t *)"ThreadProtocolLeaf",
                        sizeof("ThreadProtocolLeaf") - 1},
};

static const ScoopManagedGlobalDescriptor no_managed_globals[1];
static const ScoopImmortalObjectDescriptor no_immortal_objects[1];

typedef enum ProbeKind {
    PROBE_CALLBACK_MANAGED,
    PROBE_NATIVE_SAFE,
    PROBE_NATIVE_BORROWED,
} ProbeKind;

typedef struct ThreadProbe {
    ProbeKind kind;
    uint64_t value;
    _Atomic(uint32_t) *ready_count;
    _Atomic(uint64_t) *start_epoch;
    _Atomic(bool) *collection_finished;
    TestLeaf *old_root;
    TestLeaf *current_root;
    bool passed;
} ThreadProbe;

static TestLeaf *new_leaf(uint64_t value) {
    TestLeaf *leaf = scoop_gc_alloc_internal(&leaf_td, sizeof *leaf);
    leaf->value = value;
    return leaf;
}

static void initialize_fake_frame(uintptr_t frame[4], TestLeaf *root,
                                  uintptr_t managed_boundary) {
    memset(frame, 0, sizeof(uintptr_t) * 4);
    memcpy(&frame[0], &root, sizeof root);
    frame[2] = managed_boundary;
}

static void wait_for_collection_start(const ThreadProbe *probe) {
    uint64_t epoch;
    do {
        epoch = atomic_load_explicit(probe->start_epoch, memory_order_acquire);
        sched_yield();
    } while (epoch == UINT64_MAX);
    while (scoop_rt_thread_debug_gc_epoch() == epoch) {
        sched_yield();
    }
}

static bool validate_relocated_root(ThreadProbe *probe, TestLeaf *current,
                                    uintptr_t frame_root) {
    probe->current_root = current;
    return current != probe->old_root && current == (TestLeaf *)frame_root &&
           current->value == probe->value &&
           scoop_rt_gc_debug_is_allocated(current) &&
           !scoop_rt_gc_debug_is_allocated(probe->old_root);
}

static void run_callback_managed_probe(ThreadProbe *probe,
                                       uintptr_t managed_boundary) {
    ScoopCallbackThreadEntry callback_entry = {0};
    scoop_thread_enter_callback(&callback_entry,
                                (const void *)managed_boundary);

    TestLeaf *root = probe->old_root;
    _Alignas(16) uintptr_t frame[4];
    initialize_fake_frame(frame, root, managed_boundary);
    atomic_fetch_add_explicit(probe->ready_count, 1, memory_order_release);

    wait_for_collection_start(probe);
    scoop_rt_safepoint_impl(0x1010, (uintptr_t)&frame[0],
                            (uintptr_t)&frame[2]);
    memcpy(&root, &frame[0], sizeof root);
    probe->passed = validate_relocated_root(probe, root, frame[0]);

    scoop_thread_leave_callback(&callback_entry);
}

static void run_native_transition_probe(ThreadProbe *probe,
                                        uintptr_t managed_boundary) {
    scoop_thread_enter_managed((const void *)managed_boundary);
    TestLeaf *root = probe->old_root;

    ScoopCallerRootEntry root_entry = {
        .base = &root,
        .scan = one_root_scan,
    };
    ScoopCallerRootFrame root_frame;
    scoop_rt_push_caller_roots(&root_frame, &root_entry, 1);

    _Alignas(16) uintptr_t frame[4];
    initialize_fake_frame(frame, root, managed_boundary);
    ScoopThreadTransition transition = {0};
    if (probe->kind == PROBE_NATIVE_SAFE) {
        scoop_rt_enter_native_safe_impl(
            &transition, (uintptr_t)&frame[0], 0x1010,
            (uintptr_t)&frame[0], (uintptr_t)&frame[2]);
    } else {
        assert(probe->kind == PROBE_NATIVE_BORROWED);
        scoop_rt_enter_native_borrowed_impl(
            &transition, (uintptr_t)&frame[0], 0x1010,
            (uintptr_t)&frame[0], (uintptr_t)&frame[2]);
    }
    atomic_fetch_add_explicit(probe->ready_count, 1, memory_order_release);

    wait_for_collection_start(probe);
    if (probe->kind == PROBE_NATIVE_SAFE) {
        while (!atomic_load_explicit(probe->collection_finished,
                                     memory_order_acquire)) {
            sched_yield();
        }
        scoop_rt_leave_native_safe(&transition);
    } else {
        /* The main collector waits until this borrowed segment explicitly
         * enters the runtime and parks for the active epoch. This is the
         * coordination entry itself, not a request for a second collection. */
        scoop_thread_native_borrowed_entry();
        scoop_rt_leave_native_borrowed(&transition);
    }

    probe->passed = validate_relocated_root(probe, root, frame[0]);
    scoop_rt_pop_caller_roots(&root_frame);
    scoop_thread_leave_managed();
}

static void *run_thread_probe(void *raw_probe) {
    ThreadProbe *probe = raw_probe;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t managed_boundary = scoop_rt_thread_debug_stack_high();
    if (probe->kind == PROBE_CALLBACK_MANAGED) {
        run_callback_managed_probe(probe, managed_boundary);
    } else {
        run_native_transition_probe(probe, managed_boundary);
    }
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static TestLeaf *collect_with_stack_root(TestLeaf *root) {
    _Alignas(16) uintptr_t frame[4];
    uintptr_t managed_boundary =
        (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    initialize_fake_frame(frame, root, managed_boundary);
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)&frame[0],
                             (uintptr_t)&frame[2]);
    memcpy(&root, &frame[0], sizeof root);
    return root;
}

int main(void) {
    uintptr_t managed_boundary_marker = 0;
    scoop_thread_runtime_init();
    scoop_gc_stackmaps_init();
    scoop_gc_register_image_roots(no_managed_globals, 0,
                                  no_immortal_objects, 0);
    scoop_gc_heap_init();
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&managed_boundary_marker);

    _Atomic(uint32_t) ready_count = 0;
    _Atomic(uint64_t) start_epoch = UINT64_MAX;
    _Atomic(bool) collection_finished = false;
    TestLeaf *main_old = new_leaf(200);
    ThreadProbe probes[] = {
        {
            .kind = PROBE_CALLBACK_MANAGED,
            .value = 201,
            .ready_count = &ready_count,
            .start_epoch = &start_epoch,
            .collection_finished = &collection_finished,
            .old_root = new_leaf(201),
        },
        {
            .kind = PROBE_NATIVE_SAFE,
            .value = 202,
            .ready_count = &ready_count,
            .start_epoch = &start_epoch,
            .collection_finished = &collection_finished,
            .old_root = new_leaf(202),
        },
        {
            .kind = PROBE_NATIVE_BORROWED,
            .value = 203,
            .ready_count = &ready_count,
            .start_epoch = &start_epoch,
            .collection_finished = &collection_finished,
            .old_root = new_leaf(203),
        },
    };
    pthread_t threads[3];
    for (size_t index = 0; index < 3; index++) {
        assert(pthread_create(&threads[index], NULL, run_thread_probe,
                              &probes[index]) == 0);
    }
    while (atomic_load_explicit(&ready_count, memory_order_acquire) != 3) {
        sched_yield();
    }
    assert(scoop_rt_thread_debug_count() == 4);

    uint64_t epoch = scoop_rt_thread_debug_gc_epoch();
    atomic_store_explicit(&start_epoch, epoch, memory_order_release);
    TestLeaf *main_current = collect_with_stack_root(main_old);
    assert(main_current != main_old && main_current->value == 200);
    assert(scoop_rt_thread_debug_gc_epoch() == epoch + 1);
    assert(scoop_rt_thread_debug_last_gc_parked_count() == 2);
    assert(scoop_rt_thread_debug_last_gc_native_safe_count() == 1);
    atomic_store_explicit(&collection_finished, true, memory_order_release);

    for (size_t index = 0; index < 3; index++) {
        assert(pthread_join(threads[index], NULL) == 0);
        assert(probes[index].passed);
    }
    assert(scoop_rt_thread_debug_count() == 1);

    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("moving collector thread-protocol tests passed");
    return 0;
}
