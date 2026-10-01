#include <assert.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>

#include "../src/initialization.c"

const ScoopTypeDescriptor scoop_td_String = {
    .type_id = 1,
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
            .minimum_size = sizeof(ScoopString),
            .instance_alignment = _Alignof(ScoopString),
            .inline_offset = sizeof(ScoopString),
            .inline_size = 1,
            .inline_stride = 1,
            .inline_alignment = 1,
        },
    .diagnostic_name = {(const uint8_t *)"String", sizeof("String") - 1},
};

void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size) {
    (void)td;
    (void)size;
    abort();
}

const ScoopInitializationUnitDescriptorV1 *const scoop_image_initialization_units[] = {
    NULL};
const uint64_t scoop_image_initialization_unit_count = 0;

static void unused_initializer(void) {}
static void unused_ensure(void) {}
static uint64_t eager_ensure_count;
static uint64_t lazy_ensure_count;

static void eager_ensure(void) { eager_ensure_count++; }
static void lazy_ensure(void) { lazy_ensure_count++; }

typedef struct TestUnit {
    ScoopInitializationCell cell;
    uint64_t storage;
    void *failure;
    ScoopStaticStorageDescriptorV1 storage_registration;
    ScoopStaticStorageDescriptorV1 failure_registration;
    ScoopInitializationUnitDescriptorV1 descriptor;
} TestUnit;

static void initialize_test_unit(TestUnit *unit, uint8_t identity_byte,
                                 const char *display_name) {
    *unit = (TestUnit){
        .cell = {0},
        .storage = 0,
        .failure = NULL,
    };
    unit->storage_registration.writable_base = &unit->storage;
    unit->failure_registration.writable_base = &unit->failure;
    unit->descriptor = (ScoopInitializationUnitDescriptorV1){
        .schedule_kind = SCOOP_INITIALIZATION_LAZY_ACCESS_V1,
        .diagnostic_path = {(const uint8_t *)display_name, strlen(display_name)},
        .cell = &unit->cell,
        .storage = &unit->storage_registration,
        .failure_root = &unit->failure_registration,
        .initializer_entry = unused_initializer,
        .ensure_entry = unused_ensure,
    };
    unit->descriptor.registration.semantic_id.bytes[31] = identity_byte;
}

static void test_startup_schedule(void) {
    TestUnit lazy;
    TestUnit eager;
    initialize_test_unit(&lazy, 1, "top-level:lazy");
    initialize_test_unit(&eager, 2, "top-level:eager");
    lazy.descriptor.ensure_entry = lazy_ensure;
    eager.descriptor.schedule_kind = SCOOP_INITIALIZATION_EAGER_STARTUP_V1;
    eager.descriptor.ensure_entry = eager_ensure;
    const ScoopInitializationUnitDescriptorV1 *units[] = {
        &lazy.descriptor,
        &eager.descriptor,
    };
    eager_ensure_count = 0;
    lazy_ensure_count = 0;
    initialize_units(units, 2);
    assert(eager_ensure_count == 1);
    assert(lazy_ensure_count == 0);
}

__attribute__((noinline)) static uint64_t
managed_enter(const ScoopInitializationUnitDescriptorV1 *unit) {
    uintptr_t stack_marker = 0;
    return scoop_rt_init_enter_impl(unit, 1, (uintptr_t)&stack_marker,
                                    (uintptr_t)__builtin_frame_address(0));
}

__attribute__((noinline)) static void empty_collection(void) {
    uintptr_t stack_marker = 0;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 1, (uintptr_t)&stack_marker,
                                     (uintptr_t)__builtin_frame_address(0));
    assert(scoop_thread_begin_collection());
    scoop_thread_end_collection();
    scoop_thread_pop_managed_anchor(&anchor);
}

typedef struct WorkerPlan {
    TestUnit *owned;
    TestUnit *waited;
    _Atomic(ScoopThreadState *) state;
    _Atomic(uint64_t) result;
} WorkerPlan;

static void *wait_worker(void *context) {
    WorkerPlan *plan = context;
    assert(scoop_rt_attach_foreign_thread());
    scoop_thread_enter_managed(__builtin_frame_address(0));
    atomic_store_explicit(&plan->state, scoop_thread_current_required(),
                          memory_order_release);
    atomic_store_explicit(&plan->result, managed_enter(&plan->waited->descriptor),
                          memory_order_release);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *cycle_worker(void *context) {
    WorkerPlan *plan = context;
    assert(scoop_rt_attach_foreign_thread());
    scoop_thread_enter_managed(__builtin_frame_address(0));
    assert(managed_enter(&plan->owned->descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    atomic_store_explicit(&plan->state, scoop_thread_current_required(),
                          memory_order_release);
    uint64_t result = managed_enter(&plan->waited->descriptor);
    atomic_store_explicit(&plan->result, result, memory_order_release);
    assert(result == SCOOP_INIT_RESULT_FAILED);
    init_fail(&plan->owned->descriptor, plan->waited->failure);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void wait_for_edge(WorkerPlan *plan, TestUnit *unit) {
    for (uint64_t attempt = 0; attempt < UINT64_C(10000000); attempt++) {
        ScoopThreadState *state =
            atomic_load_explicit(&plan->state, memory_order_acquire);
        scoop_thread_registry_lock();
        bool waiting = state != NULL &&
                       state->initialization_wait == &unit->descriptor &&
                       atomic_load_explicit(&state->mode, memory_order_acquire) ==
                           SCOOP_THREAD_PARKED;
        scoop_thread_registry_unlock();
        if (waiting) {
            return;
        }
        sched_yield();
    }
    assert(!"worker did not publish its initialization wait edge");
}

static void test_ready_and_failure(void) {
    TestUnit ready;
    initialize_test_unit(&ready, 1, "top-level:ready");
    assert(managed_enter(&ready.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    init_succeed(&ready.descriptor);
    assert(managed_enter(&ready.descriptor) == SCOOP_INIT_READY);

    static uint64_t failure_object;
    TestUnit failed;
    initialize_test_unit(&failed, 2, "top-level:failed");
    assert(managed_enter(&failed.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    init_fail(&failed.descriptor, &failure_object);
    assert(managed_enter(&failed.descriptor) == SCOOP_INIT_RESULT_FAILED);
    assert(init_failure(&failed.descriptor) == &failure_object);
}

static void test_same_thread_cycle(void) {
    static uint64_t failure_object;
    TestUnit outer;
    TestUnit inner;
    initialize_test_unit(&outer, 3, "object:Outer");
    initialize_test_unit(&inner, 4, "object:Inner");
    static const uint8_t outer_path[] = "object:Outer trailing bytes";
    outer.descriptor.diagnostic_path = (ScoopByteSpanV1){outer_path, 12};
    assert(managed_enter(&outer.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    assert(managed_enter(&inner.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    assert(managed_enter(&outer.descriptor) == SCOOP_INIT_CYCLE);
    assert(strcmp(scoop_thread_current_required()->initialization_cycle_path,
                  "initialization cycle: object:Outer -> object:Inner -> "
                  "object:Outer") == 0);
    init_fail(&inner.descriptor, &failure_object);
    init_fail(&outer.descriptor, &failure_object);
}

static void test_wait_participates_in_collection(void) {
    TestUnit unit;
    initialize_test_unit(&unit, 5, "top-level:waited");
    assert(managed_enter(&unit.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    WorkerPlan plan = {
        .owned = NULL,
        .waited = &unit,
        .state = NULL,
        .result = UINT64_MAX,
    };
    pthread_t worker;
    assert(pthread_create(&worker, NULL, wait_worker, &plan) == 0);
    wait_for_edge(&plan, &unit);
    empty_collection();
    assert(atomic_load_explicit(&scoop_thread_last_gc_parked_count,
                                memory_order_acquire) == 1);
    init_succeed(&unit.descriptor);
    assert(pthread_join(worker, NULL) == 0);
    assert(atomic_load_explicit(&plan.result, memory_order_acquire) ==
           SCOOP_INIT_READY);
}

static void test_cross_thread_cycle(void) {
    static uint64_t failure_object;
    TestUnit left;
    TestUnit right;
    initialize_test_unit(&left, 6, "top-level:left");
    initialize_test_unit(&right, 7, "top-level:right");
    assert(managed_enter(&left.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    WorkerPlan plan = {
        .owned = &right,
        .waited = &left,
        .state = NULL,
        .result = UINT64_MAX,
    };
    pthread_t worker;
    assert(pthread_create(&worker, NULL, cycle_worker, &plan) == 0);
    wait_for_edge(&plan, &left);
    assert(managed_enter(&right.descriptor) == SCOOP_INIT_CYCLE);
    assert(strcmp(scoop_thread_current_required()->initialization_cycle_path,
                  "initialization cycle: top-level:left -> top-level:right -> "
                  "top-level:left") == 0);
    init_fail(&left.descriptor, &failure_object);
    assert(pthread_join(worker, NULL) == 0);
    assert(atomic_load_explicit(&plan.result, memory_order_acquire) ==
           SCOOP_INIT_RESULT_FAILED);
    assert(right.failure == &failure_object);
}

int main(void) {
    scoop_thread_runtime_init();
    scoop_thread_attach_main(__builtin_frame_address(0));
    test_startup_schedule();
    test_ready_and_failure();
    test_same_thread_cycle();
    test_wait_participates_in_collection();
    test_cross_thread_cycle();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("initialization coordinator tests passed");
    return 0;
}
