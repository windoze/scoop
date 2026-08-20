#include <assert.h>
#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "../src/gc/gc_internal.h"
#include "../src/generated_entries.h"
#include "../src/managed_entries.h"
#include "../src/task_context.h"
#include "../src/thread.h"
#include "platform/image_fixture.h"

typedef struct Payload {
    ScoopObjectHeader header;
    uint64_t value;
} Payload;

static const uint64_t task_scan[] = {1, offsetof(ScoopTaskContext, root)};
static const uint64_t node_scan[] = {4, 16, 24, 32, 40};
#define FIXED_SHAPE(type)                                                       \
    { .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,                      \
      .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,                       \
      .minimum_size = sizeof(type), .instance_alignment = _Alignof(type) }
static const ScoopTypeDescriptor task_td = {
    .type_id = 1, .instance_shape = FIXED_SHAPE(ScoopTaskContext),
    .object_scan = task_scan,
};
static const ScoopTypeDescriptor node_td = {
    .type_id = 2, .instance_shape = FIXED_SHAPE(ScoopContextNode),
    .object_scan = node_scan,
};
static const ScoopTypeDescriptor payload_td = {
    .type_id = 3, .instance_shape = FIXED_SHAPE(Payload),
};

static void frame_init(uintptr_t frame[4]) {
    frame[0] = frame[1] = frame[3] = 0;
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
}

static void *push(uint64_t cell, uint64_t value) {
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame);
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame,
                                      (uintptr_t)&frame[2]);
    Payload *payload = scoop_gc_alloc_internal(&payload_td, sizeof *payload);
    payload->value = value;
    scoop_thread_pop_managed_anchor(&anchor);
    return scoop_rt_context_push_impl(&cell, payload, &node_td, 0x1010,
                                       (uintptr_t)frame, (uintptr_t)&frame[2]);
}

static uint64_t lookup(uint64_t cell) {
    Payload *payload = scoop_rt_context_try_get(&cell);
    assert(payload != NULL);
    return payload->value;
}

static void collect(void) {
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame);
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
}

static void *collect_on_foreign_thread(void *unused) {
    (void)unused;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = 0;
    scoop_thread_enter_managed(&boundary);
    collect();
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void exercise_context(void) {
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame);
    void *task = scoop_rt_context_ensure_root_impl(
        &task_td, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    void *snapshot = NULL, *child = NULL, *guard = NULL, *mark = NULL;
    void *marks[7] = {0};
    void **slots[12] = {&task, &snapshot, &child, &guard, &mark};
    for (size_t index = 0; index < 7; index++) slots[index + 5] = &marks[index];
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 12);
    const uint64_t cells[] = {1, 2, 4, 5, 16, 17, UINT64_C(1) << 32};
    uint64_t absent = 18;
    assert(scoop_rt_context_try_get(&absent) == NULL);
    for (size_t index = 0; index < 7; index++) {
        marks[index] = push(cells[index], 100 + index);
        for (size_t prior = 0; prior <= index; prior++) {
            assert(lookup(cells[prior]) == 100 + prior);
        }
    }
    snapshot = scoop_rt_context_snapshot();
    child = scoop_rt_context_fork_impl(snapshot, &task_td, 0x1010,
                                      (uintptr_t)frame, (uintptr_t)&frame[2]);
    assert(child != task && ((ScoopTaskContext *)child)->root == snapshot);
    guard = scoop_rt_context_enter(child);
    assert(guard == task);
    mark = push(cells[0], 999);
    collect();
    assert(lookup(cells[0]) == 999 && lookup(cells[6]) == 106);
    assert(((ScoopTaskContext *)task)->root == snapshot);
    scoop_rt_context_leave(guard);
    assert(lookup(cells[0]) == 100);
    scoop_rt_context_restore(child, mark);
    assert(((ScoopTaskContext *)child)->root == snapshot);
    for (size_t index = 7; index != 0; index--) {
        scoop_rt_context_restore(task, marks[index - 1]);
        collect();
        assert(scoop_rt_context_try_get(&cells[index - 1]) == NULL);
        for (size_t prior = 0; prior + 1 < index; prior++) {
            assert(lookup(cells[prior]) == 100 + prior);
        }
    }
    assert(scoop_rt_context_snapshot() == NULL);
    assert(scoop_rt_context_ensure_root_impl(
        &task_td, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]) == task);
    (void)push(cells[6], 1234);
    scoop_rt_pop_native_roots(&roots);
}

int main(void) {
    assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&task_td, &node_td, &payload_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    ScoopImageRegistry registry = *scoop_image_current();
    scoop_image_unpublish();
    registry.context_key_count = UINT64_C(1) << 32;
    registry.context_height = 16;
    scoop_image_publish(&registry);
    scoop_thread_attach_main();
    uintptr_t boundary = 0;
    scoop_thread_enter_managed(&boundary);
    exercise_context();
    uintptr_t previous = (uintptr_t)scoop_rt_context_current();
    scoop_thread_leave_managed();
    pthread_t collector;
    assert(pthread_create(&collector, NULL, collect_on_foreign_thread, NULL) == 0);
    assert(pthread_join(collector, NULL) == 0);
    scoop_thread_enter_managed(&boundary);
    assert((uintptr_t)scoop_rt_context_current() != previous);
    assert(lookup(UINT64_C(1) << 32) == 1234);
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("task context radix, fork, restore and moving roots passed");
    return 0;
}
