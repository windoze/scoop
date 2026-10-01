#include <assert.h>

#include "../src/initialization.c"
#include "platform/image_fixture.h"

enum { WAITER_COUNT = 3, UNIT_COUNT = 2 };

typedef struct Payload {
    ScoopObjectHeader header;
    uint64_t value;
} Payload;

static const ScoopTypeDescriptor payload_type = {
    .type_id = 1,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(Payload),
                       .instance_alignment = _Alignof(Payload)},
};
const ScoopTypeDescriptor scoop_td_String = {
    .type_id = 2,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = sizeof(ScoopString),
                       .instance_alignment = _Alignof(ScoopString),
                       .inline_offset = sizeof(ScoopString),
                       .inline_size = 1,
                       .inline_stride = 1,
                       .inline_alignment = 1},
};

typedef struct TestUnit {
    ScoopInitializationCell cell;
    Payload *storage, *failure;
    ScoopStaticStorageDescriptorV1 storage_record, failure_record;
    ScoopInitializationUnitDescriptorV1 descriptor;
} TestUnit;

static TestUnit units[UNIT_COUNT];

static void publish_fixture(void) {
    static const uint64_t scan[] = {1, 0};
    static ScoopImageRegistry registry;
    static ScoopTypeRegistrationDescriptorV1 type = {.descriptor = &payload_type};
    static ScoopRegisteredRecord type_record = {.record = &type};
    static ScoopRecordAddress type_address = {(uintptr_t)&payload_type, 0};
    static ScoopRegisteredRecord records[UNIT_COUNT];
    static ScoopRecordAddress addresses[UNIT_COUNT];
    static const ScoopStaticStorageDescriptorV1 *roots[UNIT_COUNT * 2];
    for (size_t index = 0; index < UNIT_COUNT; index++) {
        TestUnit *unit = &units[index];
        unit->storage_record = (ScoopStaticStorageDescriptorV1){
            .writable_base = &unit->storage, .scan_program = scan};
        unit->failure_record = (ScoopStaticStorageDescriptorV1){
            .writable_base = &unit->failure, .scan_program = scan};
        unit->descriptor = (ScoopInitializationUnitDescriptorV1){
            .cell = &unit->cell,
            .storage = &unit->storage_record,
            .failure_root = &unit->failure_record,
        };
        records[index].record = &unit->descriptor;
        addresses[index] = (ScoopRecordAddress){(uintptr_t)&unit->descriptor, index};
        roots[index * 2] = &unit->storage_record;
        roots[index * 2 + 1] = &unit->failure_record;
    }
    registry.tables[SCOOP_RECORD_TYPE] =
        (ScoopRecordTable){.entries = &type_record, .count = 1};
    registry.tables[SCOOP_RECORD_UNIT] =
        (ScoopRecordTable){records, addresses, UNIT_COUNT};
    registry.type_addresses = &type_address;
    registry.static_roots = roots;
    registry.static_root_count = UNIT_COUNT * 2;
    registry.stackmaps = scoop_test_stackmaps();
    scoop_image_publish(&registry);
    scoop_gc_stackmaps_init(&registry.stackmaps);
    scoop_gc_heap_init();
}

static void frame_init(uintptr_t frame[4]) {
    memset(frame, 0, 4 * sizeof *frame);
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
}

static uint64_t enter(TestUnit *unit) {
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame);
    return scoop_rt_init_enter_impl(&unit->descriptor, 0x1010, (uintptr_t)frame,
                                    (uintptr_t)&frame[2]);
}

static void collect(void) {
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame);
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
}

typedef struct Waiter {
    TestUnit *unit;
    ScoopThreadState *state;
    uint64_t result, value;
} Waiter;

static void *waiter_run(void *context) {
    Waiter *waiter = context;
    assert(scoop_rt_attach_foreign_thread());
    scoop_thread_enter_managed(__builtin_frame_address(0));
    scoop_thread_registry_lock();
    waiter->state = scoop_thread_current_required();
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
    waiter->result = enter(waiter->unit);
    assert(waiter->result != SCOOP_INIT_RUN_INITIALIZER);
    Payload *value = waiter->result == SCOOP_INIT_READY
                         ? waiter->unit->storage
                         : init_failure(&waiter->unit->descriptor);
    waiter->value = value->value;
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void wait_until_parked(Waiter waiters[WAITER_COUNT]) {
    scoop_thread_registry_lock();
    for (;;) {
        size_t count = 0;
        for (size_t index = 0; index < WAITER_COUNT; index++) {
            ScoopThreadState *state = waiters[index].state;
            if (state != NULL &&
                state->initialization_wait == &waiters[index].unit->descriptor &&
                atomic_load_explicit(&state->mode, memory_order_acquire) ==
                    SCOOP_THREAD_PARKED) {
                count++;
            }
        }
        if (count == WAITER_COUNT) {
            break;
        }
        scoop_thread_world_wait();
    }
    scoop_thread_registry_unlock();
}

static void test_waiters(TestUnit *unit, bool fail) {
    assert(enter(unit) == SCOOP_INIT_RUN_INITIALIZER);
    _Alignas(16) uintptr_t frame[4];
    frame_init(frame);
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame,
                                     (uintptr_t)&frame[2]);
    unit->storage = scoop_gc_alloc_internal(&payload_type, sizeof(Payload));
    scoop_thread_pop_managed_anchor(&anchor);
    unit->storage->value = 42;
    uintptr_t before = (uintptr_t)unit->storage;
    Waiter waiters[WAITER_COUNT] = {0};
    pthread_t threads[WAITER_COUNT];
    for (size_t index = 0; index < WAITER_COUNT; index++) {
        waiters[index].unit = unit;
        assert(pthread_create(&threads[index], NULL, waiter_run, &waiters[index]) == 0);
    }
    wait_until_parked(waiters);
    collect();
    assert((uintptr_t)unit->storage != before && unit->storage->value == 42);
    assert(atomic_load_explicit(&scoop_thread_last_gc_parked_count,
                                memory_order_acquire) == WAITER_COUNT);
    if (fail) {
        init_fail(&unit->descriptor, unit->storage);
        unit->storage = NULL;
    } else {
        init_succeed(&unit->descriptor);
    }
    const void *boundary = scoop_thread_current_required()->managed_stack_boundary;
    scoop_thread_leave_managed();
    for (size_t index = 0; index < WAITER_COUNT; index++) {
        assert(pthread_join(threads[index], NULL) == 0);
        assert(waiters[index].result ==
               (fail ? SCOOP_INIT_RESULT_FAILED : SCOOP_INIT_READY));
        assert(waiters[index].value == 42);
    }
    scoop_thread_enter_managed(boundary);
    before = (uintptr_t)(fail ? unit->failure : unit->storage);
    collect();
    Payload *value = fail ? init_failure(&unit->descriptor) : unit->storage;
    assert((uintptr_t)value != before && value->value == 42);
    assert(enter(unit) == (fail ? SCOOP_INIT_RESULT_FAILED : SCOOP_INIT_READY));
    assert(unit->cell.owner_thread == NULL);
}

int main(void) {
    assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
    scoop_thread_runtime_init();
    publish_fixture();
    scoop_thread_attach_main();
    scoop_thread_enter_managed(__builtin_frame_address(0));
    test_waiters(&units[0], false);
    test_waiters(&units[1], true);
    assert(scoop_thread_current_required()->initialization_stack_len == 0);
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("initialization waiters and moving GC tests passed");
    return 0;
}
