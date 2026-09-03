#include "support.h"

static const uint64_t callback_signature_i64;
static const uint64_t callback_signature_other;
static _Atomic bool callback_lease_entered;
static _Atomic bool callback_lease_leave;

static uint64_t callback_add_adapter(const void *raw_closure,
                                     void *result_storage,
                                     const void *const *argument_storage,
                                     void **exception_out) {
    (void)exception_out;
    const ScoopNode *closure = raw_closure;
    int64_t argument = *(const int64_t *)argument_storage[0];
    ScoopNode *allocated = new_node(closure->value + argument, NULL);
    *(int64_t *)result_storage = allocated->value;
    scoop_rt_gc_collect();
    return SCOOP_FOREIGN_CALLBACK_RETURNED;
}

static uint64_t callback_throw_adapter(const void *raw_closure,
                                       void *result_storage,
                                       const void *const *argument_storage,
                                       void **exception_out) {
    (void)result_storage;
    (void)argument_storage;
    const ScoopNode *closure = raw_closure;
    *exception_out = new_node(closure->value + 1000, NULL);
    return SCOOP_FOREIGN_CALLBACK_THREW;
}

static uint64_t callback_lease_adapter(const void *raw_closure,
                                       void *result_storage,
                                       const void *const *argument_storage,
                                       void **exception_out) {
    (void)exception_out;
    const ScoopNode *closure = raw_closure;
    int64_t argument = *(const int64_t *)argument_storage[0];
    atomic_store_explicit(&callback_lease_entered, true, memory_order_release);
    while (!atomic_load_explicit(&callback_lease_leave, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    *(int64_t *)result_storage = closure->value + argument;
    return SCOOP_FOREIGN_CALLBACK_RETURNED;
}

typedef struct CallbackInvokeProbe {
    void *context;
    const void *signature;
    int64_t argument;
    int64_t result;
    uint32_t status;
    bool detached_before;
    bool detached_after;
} CallbackInvokeProbe;

static void *invoke_callback_worker(void *raw_probe) {
    CallbackInvokeProbe *probe = raw_probe;
    const void *arguments[] = {&probe->argument};
    probe->detached_before = !scoop_rt_thread_debug_is_attached();
    probe->status = scoop_runtime_callback_invoke(
        probe->context, probe->signature, &probe->result, arguments);
    probe->detached_after = !scoop_rt_thread_debug_is_attached();
    return NULL;
}

static bool join_thread_native_safe(pthread_t thread) {
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition transition = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&transition,
                               (uintptr_t)&managed_stack_pointer);
    int result = pthread_join(thread, NULL);
    scoop_rt_leave_native_safe(&transition);
    scoop_rt_pop_caller_roots(&caller_frame);
    return result == 0;
}

void run_callback_tests(void) {
    /* Managed callback gateway: a one-shot token transfers its worker owner,
     * automatically attaches a foreign pthread, executes an allocating
     * adapter (including STW GC), records completion, and detaches again. */
    ScoopNode *callback_closure = new_node(40, NULL);
    void *one_shot = scoop_runtime_callback_register(
        callback_closure, callback_add_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_ONE_SHOT);
    void *one_shot_observer = scoop_runtime_callback_retain(one_shot);
    CallbackInvokeProbe callback_probe = {
        .context = one_shot,
        .signature = &callback_signature_i64,
        .argument = 2,
    };
    pthread_t callback_thread;
    bool callback_created =
        pthread_create(&callback_thread, NULL, invoke_callback_worker,
                       &callback_probe) == 0;
    bool callback_joined =
        callback_created && join_thread_native_safe(callback_thread);
    scoop_rt_println_boolean(
        callback_joined && callback_probe.detached_before &&
        callback_probe.detached_after &&
        callback_probe.status == SCOOP_FOREIGN_CALLBACK_RETURNED &&
        callback_probe.result == 42 &&
        scoop_runtime_callback_state(one_shot_observer) ==
            SCOOP_FOREIGN_CALLBACK_COMPLETED &&
        scoop_runtime_callback_failure(one_shot_observer) == NULL);

    /* The claimed one-shot cannot be entered a second time while its observer
     * keeps the stale boundary deterministic. */
    fflush(stdout);
    pid_t callback_pid = fork();
    if (callback_pid == 0) {
        const void *arguments[] = {&callback_probe.argument};
        (void)scoop_runtime_callback_invoke(
            one_shot, &callback_signature_i64, &callback_probe.result,
            arguments);
        _exit(0);
    }
    int callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));
    scoop_runtime_callback_release(one_shot_observer);

    /* Exception status never crosses the C frame. The first managed failure
     * is rooted by a handle until the observer reads and releases it. */
    ScoopNode *throw_closure = new_node(7, NULL);
    void *throwing = scoop_runtime_callback_register(
        throw_closure, callback_throw_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_ONE_SHOT);
    void *throw_observer = scoop_runtime_callback_retain(throwing);
    CallbackInvokeProbe throw_probe = {
        .context = throwing,
        .signature = &callback_signature_i64,
    };
    pthread_t throw_thread;
    bool throw_created = pthread_create(&throw_thread, NULL,
                                        invoke_callback_worker,
                                        &throw_probe) == 0;
    bool throw_joined = throw_created && join_thread_native_safe(throw_thread);
    const ScoopNode *callback_failure =
        scoop_runtime_callback_failure(throw_observer);
    scoop_rt_println_boolean(
        throw_joined && throw_probe.status == SCOOP_FOREIGN_CALLBACK_THREW &&
        throw_probe.result == 0 &&
        scoop_runtime_callback_state(throw_observer) ==
            SCOOP_FOREIGN_CALLBACK_FAILED &&
        callback_failure != NULL && callback_failure->value == 1007);
    scoop_runtime_callback_release(throw_observer);

    /* Reusable tokens support concurrent workers and same-thread re-entry
     * from an existing native-safe transition. */
    ScoopNode *reusable_closure = new_node(50, NULL);
    void *reusable = scoop_runtime_callback_register(
        reusable_closure, callback_add_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_REUSABLE);
    CallbackInvokeProbe reusable_probes[2] = {
        {.context = reusable,
         .signature = &callback_signature_i64,
         .argument = 1},
        {.context = reusable,
         .signature = &callback_signature_i64,
         .argument = 2},
    };
    pthread_t reusable_threads[2];
    bool reusable_ok = true;
    for (size_t i = 0; i < 2; i++) {
        reusable_ok =
            pthread_create(&reusable_threads[i], NULL, invoke_callback_worker,
                           &reusable_probes[i]) == 0 &&
            reusable_ok;
    }
    for (size_t i = 0; i < 2; i++) {
        reusable_ok = join_thread_native_safe(reusable_threads[i]) &&
                      reusable_probes[i].result == 51 + (int64_t)i &&
                      reusable_ok;
    }
    ScoopCallerRootFrame callback_caller_frame;
    ScoopThreadTransition callback_transition = {0};
    volatile char callback_stack_pointer = 0;
    scoop_rt_push_caller_roots(&callback_caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&callback_transition,
                               (uintptr_t)&callback_stack_pointer);
    int64_t nested_argument = 3;
    int64_t nested_result = 0;
    const void *nested_arguments[] = {&nested_argument};
    uint32_t nested_status = scoop_runtime_callback_invoke(
        reusable, &callback_signature_i64, &nested_result, nested_arguments);
    scoop_rt_leave_native_safe(&callback_transition);
    scoop_rt_pop_caller_roots(&callback_caller_frame);
    reusable_ok = reusable_ok && nested_status == SCOOP_FOREIGN_CALLBACK_RETURNED &&
                  nested_result == 53 &&
                  scoop_runtime_callback_state(reusable) ==
                      SCOOP_FOREIGN_CALLBACK_REGISTERED;
    scoop_rt_println_boolean(reusable_ok);

    /* The active invocation is an independent lease. Deterministically hold
     * the adapter open, drop every external owner, and prove that the token
     * remains live until the invocation returns. */
    atomic_store_explicit(&callback_lease_entered, false,
                          memory_order_release);
    atomic_store_explicit(&callback_lease_leave, false,
                          memory_order_release);
    ScoopNode *lease_closure = new_node(70, NULL);
    void *leased = scoop_runtime_callback_register(
        lease_closure, callback_lease_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_REUSABLE);
    void *lease_observer = scoop_runtime_callback_retain(leased);
    CallbackInvokeProbe lease_probe = {
        .context = leased,
        .signature = &callback_signature_i64,
        .argument = 2,
    };
    pthread_t lease_thread;
    bool lease_created =
        pthread_create(&lease_thread, NULL, invoke_callback_worker,
                       &lease_probe) == 0;
    while (lease_created &&
           !atomic_load_explicit(&callback_lease_entered,
                                 memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    bool active_lease_visible =
        lease_created &&
        scoop_runtime_callback_state(leased) ==
            SCOOP_FOREIGN_CALLBACK_ACTIVE &&
        scoop_runtime_callback_debug_owner_count(leased) == 2 &&
        scoop_runtime_callback_debug_active_count(leased) == 1;
    scoop_runtime_callback_release(leased);
    scoop_runtime_callback_release(lease_observer);
    active_lease_visible =
        active_lease_visible &&
        scoop_runtime_callback_debug_owner_count(leased) == 0 &&
        scoop_runtime_callback_debug_active_count(leased) == 1 &&
        scoop_runtime_callback_debug_live_count() == 2;
    atomic_store_explicit(&callback_lease_leave, true, memory_order_release);
    bool lease_joined = lease_created && join_thread_native_safe(lease_thread);
    scoop_rt_println_boolean(
        active_lease_visible && lease_joined &&
        lease_probe.status == SCOOP_FOREIGN_CALLBACK_RETURNED &&
        lease_probe.result == 72 && lease_probe.detached_before &&
        lease_probe.detached_after &&
        scoop_runtime_callback_debug_live_count() == 1);

    /* Signature identity is checked before the token is touched by managed
     * code; a mismatched trampoline/token pair is a boundary error. */
    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        (void)scoop_runtime_callback_invoke(
            reusable, &callback_signature_other, &nested_result,
            nested_arguments);
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));
    uintptr_t stale_callback_cookie = (uintptr_t)reusable;
    scoop_runtime_callback_release(reusable);
    scoop_rt_println_boolean(scoop_runtime_callback_debug_live_count() == 0);

    /* Final release invalidates the cookie, and a reused slot advances its
     * generation instead of aliasing the previous token. */
    void *replacement = scoop_runtime_callback_register(
        reusable_closure, callback_add_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_REUSABLE);
    uintptr_t replacement_cookie = (uintptr_t)replacement;
    const uintptr_t callback_slot_mask = (UINT64_C(1) << 24) - 1;
    scoop_rt_println_boolean(
        (stale_callback_cookie & callback_slot_mask) ==
            (replacement_cookie & callback_slot_mask) &&
        stale_callback_cookie != replacement_cookie);
    scoop_runtime_callback_release(replacement);

    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        (void)scoop_runtime_callback_invoke(
            (void *)stale_callback_cookie, &callback_signature_i64,
            &nested_result, nested_arguments);
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));

    /* Shutdown rejects both a live token and a subsequent registration.
     * Each check runs in a child so the parent can finish the unit suite. */
    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        (void)scoop_runtime_callback_register(
            reusable_closure, callback_add_adapter, &callback_signature_i64,
            SCOOP_FOREIGN_CALLBACK_REUSABLE);
        scoop_callback_prepare_shutdown();
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));

    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        scoop_callback_prepare_shutdown();
        (void)scoop_runtime_callback_register(
            reusable_closure, callback_add_adapter, &callback_signature_i64,
            SCOOP_FOREIGN_CALLBACK_REUSABLE);
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));
}
