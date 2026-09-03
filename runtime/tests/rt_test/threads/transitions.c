#include "internal.h"

void run_thread_transition_tests(void) {
    /* Native transition ABI: caller roots remain published while the active
     * managed segment is frozen. native-safe returns without participating in
     * GC; native-borrowed may explicitly enter the runtime, collect using its
     * caller/native roots, and resume in borrowed mode before the LIFO leave. */
    void *caller_root_value = NULL;
    void **caller_root_slots[] = {&caller_root_value};
    static const uint64_t caller_root_scan[] = {1, 0};
    ScoopCallerRootEntry caller_root_entries[] = {
        {.base = &caller_root_value, .scan = caller_root_scan},
    };
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition safe_transition = {0};
    volatile char safe_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_safe(&safe_transition, (uintptr_t)&safe_stack_pointer);
    bool native_safe_active = scoop_rt_thread_debug_transition_depth() == 1 &&
                              scoop_rt_thread_debug_caller_root_count() == 1;
    scoop_rt_leave_native_safe(&safe_transition);
    scoop_rt_pop_caller_roots(&caller_frame);
    scoop_rt_println_boolean(native_safe_active &&
                             scoop_rt_thread_debug_transition_depth() == 0 &&
                             scoop_rt_thread_debug_caller_root_count() == 0);

    ScoopNode *compiler_root_value = new_node(16180, NULL);
    ScoopCallerRootEntry compiler_root_entries[] = {
        {.base = &compiler_root_value, .scan = caller_root_scan},
    };
    ScoopCompilerRootFrame compiler_frame;
    scoop_rt_push_compiler_roots(&compiler_frame, compiler_root_entries, 1);
    scoop_rt_gc_collect();
    bool compiler_root_survived =
        scoop_rt_thread_debug_compiler_root_count() == 1 &&
        scoop_rt_gc_debug_is_allocated(compiler_root_value) &&
        compiler_root_value->value == 16180;
    scoop_rt_pop_compiler_roots(&compiler_frame);
    scoop_rt_println_boolean(
        compiler_root_survived &&
        scoop_rt_thread_debug_compiler_root_count() == 0);

    ScoopCallerRootFrame borrowed_caller_frame;
    ScoopThreadTransition borrowed_transition = {0};
    volatile char borrowed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&borrowed_caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_borrowed(&borrowed_transition,
                                    (uintptr_t)&borrowed_stack_pointer);
    ScoopNativeRootFrame borrowed_native_frame;
    scoop_rt_push_native_roots(&borrowed_native_frame, caller_root_slots, 1);
    uint64_t borrowed_epoch = scoop_rt_thread_debug_gc_epoch();
    scoop_rt_gc_collect();
    bool borrowed_collected = scoop_rt_thread_debug_gc_epoch() == borrowed_epoch + 1 &&
                              scoop_rt_thread_debug_transition_depth() == 1 &&
                              scoop_rt_thread_debug_caller_root_count() == 1 &&
                              scoop_rt_gc_debug_native_root_count() == 1;
    scoop_rt_pop_native_roots(&borrowed_native_frame);
    scoop_rt_leave_native_borrowed(&borrowed_transition);
    scoop_rt_pop_caller_roots(&borrowed_caller_frame);
    scoop_rt_println_boolean(borrowed_collected &&
                             scoop_rt_thread_debug_transition_depth() == 0 &&
                             scoop_rt_thread_debug_caller_root_count() == 0 &&
                             scoop_rt_gc_debug_native_root_count() == 0);
}
