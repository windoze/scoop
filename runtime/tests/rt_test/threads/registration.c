#include "internal.h"

typedef struct ThreadProbe {
    bool detached_before;
    bool owns_attachment;
    bool nested_is_borrowed;
    bool stack_bounds_contain_local;
    bool registry_has_main_and_worker;
    bool native_roots_are_thread_local;
    bool mode_transitions_are_exact;
    bool detached_after;
} ThreadProbe;

static void *probe_foreign_thread(void *raw_probe) {
    ThreadProbe *probe = raw_probe;
    int stack_local = 0;
    volatile char managed_stack_boundary = 0;
    probe->detached_before = !scoop_rt_thread_debug_is_attached();
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    probe->nested_is_borrowed = !scoop_rt_attach_foreign_thread();
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;
    uintptr_t stack_low = scoop_rt_thread_debug_stack_low();
    uintptr_t stack_high = scoop_rt_thread_debug_stack_high();
    uintptr_t local_address = (uintptr_t)&stack_local;
    probe->stack_bounds_contain_local =
        stack_low <= local_address && local_address < stack_high;
    probe->registry_has_main_and_worker = scoop_rt_thread_debug_count() == 2;

    void *root_value = NULL;
    void **root_slots[] = {&root_value};
    ScoopNativeRootFrame frame;
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    scoop_rt_push_native_roots(&frame, root_slots, 1);
    probe->native_roots_are_thread_local = scoop_rt_gc_debug_native_root_count() == 1;
    scoop_rt_pop_native_roots(&frame);
    probe->native_roots_are_thread_local =
        probe->native_roots_are_thread_local && scoop_rt_gc_debug_native_root_count() == 0;
    scoop_rt_thread_debug_leave_managed();
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;

    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    probe->detached_after = !scoop_rt_thread_debug_is_attached();
    return NULL;
}

static void *detach_with_native_root(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    void *root_value = NULL;
    void **root_slots[] = {&root_value};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, root_slots, 1);
    scoop_rt_thread_debug_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *detach_twice(void *unused) {
    (void)unused;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_detach_foreign_thread();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *caller_root_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    ScoopCallerRootFrame outer;
    ScoopCallerRootFrame inner;
    scoop_rt_push_caller_roots(&outer, NULL, 0);
    scoop_rt_push_caller_roots(&inner, NULL, 0);
    scoop_rt_pop_caller_roots(&outer);
    return NULL;
}

static void *compiler_root_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    ScoopCompilerRootFrame outer;
    ScoopCompilerRootFrame inner;
    scoop_rt_push_compiler_roots(&outer, NULL, 0);
    scoop_rt_push_compiler_roots(&inner, NULL, 0);
    scoop_rt_pop_compiler_roots(&outer);
    return NULL;
}

static void leave_wrong_transition(void) {
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition active = {0};
    ScoopThreadTransition wrong = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&active, (uintptr_t)&managed_stack_pointer);
    scoop_rt_leave_native_safe(&wrong);
}

static void *transition_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    leave_wrong_transition();
    return NULL;
}

static bool thread_protocol_aborts(void *(*start)(void *)) {
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        pthread_t thread;
        if (pthread_create(&thread, NULL, start, NULL) != 0) {
            _exit(2);
        }
        (void)pthread_join(thread, NULL);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    return WIFSIGNALED(status);
}


void run_thread_registration_tests(void) {
    /* M13 thread-registration baseline: main and foreign pthreads use the
     * same TLS state/registry, nested attach does not transfer ownership,
     * stack bounds come from pthread APIs, and native roots belong to the
     * current thread state. This first probe only checks registration; later
     * probes exercise concurrent allocation and collection. */
    int main_stack_local = 0;
    uintptr_t main_stack_low = scoop_rt_thread_debug_stack_low();
    uintptr_t main_stack_high = scoop_rt_thread_debug_stack_high();
    uintptr_t main_local_address = (uintptr_t)&main_stack_local;
    scoop_rt_println_boolean(scoop_rt_thread_debug_is_attached() &&
                             scoop_rt_thread_debug_mode() ==
                                 SCOOP_THREAD_DEBUG_MANAGED &&
                             scoop_rt_thread_debug_count() == 1 &&
                             main_stack_low <= main_local_address &&
                             main_local_address < main_stack_high);
    ThreadProbe thread_probe = {0};
    pthread_t probe_thread;
    bool probe_created =
        pthread_create(&probe_thread, NULL, probe_foreign_thread, &thread_probe) == 0;
    bool probe_joined = probe_created && pthread_join(probe_thread, NULL) == 0;
    scoop_rt_println_boolean(probe_joined && thread_probe.detached_before &&
                             thread_probe.owns_attachment && thread_probe.nested_is_borrowed &&
                             thread_probe.stack_bounds_contain_local &&
                             thread_probe.registry_has_main_and_worker &&
                             thread_probe.mode_transitions_are_exact);
    scoop_rt_println_boolean(probe_joined && thread_probe.native_roots_are_thread_local);
    scoop_rt_println_boolean(probe_joined && thread_probe.detached_after &&
                             scoop_rt_thread_debug_count() == 1);
    scoop_rt_println_boolean(thread_protocol_aborts(detach_with_native_root));
    scoop_rt_println_boolean(thread_protocol_aborts(detach_twice));
    scoop_rt_println_boolean(thread_protocol_aborts(caller_root_lifo_violation));
    scoop_rt_println_boolean(thread_protocol_aborts(compiler_root_lifo_violation));
    scoop_rt_println_boolean(thread_protocol_aborts(transition_lifo_violation));
}
