#include <assert.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>

#include "../src/initialization.c"

const ScoopTypeDescriptor scoop_td_String = {
    1, 24, 8, NULL, NULL, NULL, NULL, 0, "String"};

void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size) {
    (void)td;
    (void)size;
    abort();
}

const ScoopInitializationUnitDescriptor scoop_image_initialization_units[] = {{0}};
const uint64_t scoop_image_initialization_unit_count = 0;

static void unused_initializer(void) {}
static void unused_ensure(void) {}

typedef struct TestUnit {
    ScoopInitializationCell cell;
    uint64_t storage;
    void *failure;
    ScoopInitializationUnitDescriptor descriptor;
} TestUnit;

static void initialize_test_unit(TestUnit *unit, const char *key) {
    *unit = (TestUnit){
        .cell = {0},
        .storage = 0,
        .failure = NULL,
    };
    unit->descriptor = (ScoopInitializationUnitDescriptor){
        .stable_key = key,
        .cell = &unit->cell,
        .storage = &unit->storage,
        .failure_root = &unit->failure,
        .initializer_entry = unused_initializer,
        .ensure_entry = unused_ensure,
    };
}

__attribute__((noinline)) static uint64_t managed_enter(
    const ScoopInitializationUnitDescriptor *unit) {
    uintptr_t stack_marker = 0;
    return scoop_rt_init_enter_impl(
        unit, 1, (uintptr_t)&stack_marker,
        (uintptr_t)__builtin_frame_address(0));
}

__attribute__((noinline)) static void empty_collection(void) {
    uintptr_t stack_marker = 0;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(
        &anchor, 1, (uintptr_t)&stack_marker,
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
    initialize_test_unit(&ready, "ready");
    assert(managed_enter(&ready.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    init_succeed(&ready.descriptor);
    assert(managed_enter(&ready.descriptor) == SCOOP_INIT_READY);

    static uint64_t failure_object;
    TestUnit failed;
    initialize_test_unit(&failed, "failed");
    assert(managed_enter(&failed.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    init_fail(&failed.descriptor, &failure_object);
    assert(managed_enter(&failed.descriptor) == SCOOP_INIT_RESULT_FAILED);
    assert(init_failure(&failed.descriptor) == &failure_object);
}

static void test_same_thread_cycle(void) {
    static uint64_t failure_object;
    TestUnit outer;
    TestUnit inner;
    initialize_test_unit(&outer, "outer");
    initialize_test_unit(&inner, "inner");
    assert(managed_enter(&outer.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    assert(managed_enter(&inner.descriptor) == SCOOP_INIT_RUN_INITIALIZER);
    assert(managed_enter(&outer.descriptor) == SCOOP_INIT_CYCLE);
    assert(strcmp(scoop_thread_current_required()->initialization_cycle_path,
                  "initialization cycle: outer -> inner -> outer") == 0);
    init_fail(&inner.descriptor, &failure_object);
    init_fail(&outer.descriptor, &failure_object);
}

static void test_wait_participates_in_collection(void) {
    TestUnit unit;
    initialize_test_unit(&unit, "waited");
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
    initialize_test_unit(&left, "left");
    initialize_test_unit(&right, "right");
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
                  "initialization cycle: left -> right -> left") == 0);
    init_fail(&left.descriptor, &failure_object);
    assert(pthread_join(worker, NULL) == 0);
    assert(atomic_load_explicit(&plan.result, memory_order_acquire) ==
           SCOOP_INIT_RESULT_FAILED);
    assert(right.failure == &failure_object);
}

int main(void) {
    scoop_thread_runtime_init();
    scoop_thread_attach_main(__builtin_frame_address(0));
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
