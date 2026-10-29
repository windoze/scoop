#include "throwable.h"

#include <stdlib.h>
#include <string.h>

#include "gc.h"
#include "managed_entries.h"
#include "stack_trace.h"
#include "thread.h"
#include "value_shape.h"

/* Keep the capture buffer and its stack probe off the disabled/rethrow path. */
__attribute__((noinline)) static void *prepare_trace(ScoopThrowable *exception,
                                                     const ScoopTypeDescriptor *trace_bytes_td,
                                                     uintptr_t return_pc, uintptr_t stack_pointer,
                                                     uintptr_t frame_pointer) {
    void **slots[] = {(void **)&exception};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    ScoopCallerRootFrame caller_roots;
    scoop_rt_push_caller_roots(&caller_roots, NULL, 0);
    ScoopThreadTransition transition;
    scoop_rt_enter_native_safe_impl(&transition, stack_pointer, return_pc, stack_pointer,
                                    frame_pointer);
    ScoopTraceFrames frames;
    scoop_trace_capture_frames(&frames, return_pc);
    ScoopTraceBuffer trace = scoop_trace_complete(&frames);
    scoop_rt_leave_native_safe(&transition);
    scoop_rt_pop_caller_roots(&caller_roots);

    /* The native root was updated while our managed caller was frozen. */
    if (__atomic_load_n(&exception->snapshot, __ATOMIC_ACQUIRE) == NULL) {
        ScoopManagedAnchor anchor;
        scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
        size_t size = scoop_shape_allocation_size(trace_bytes_td, trace.length);
        ScoopArray *snapshot = scoop_gc_alloc_internal(trace_bytes_td, size);
        snapshot->size = trace.length;
        memcpy(snapshot->elements, trace.bytes, trace.length);
        ScoopArray *expected = NULL;
        if (__atomic_compare_exchange_n(&exception->snapshot, &expected, snapshot, false,
                                        __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE)) {
            scoop_rt_gc_write_barrier(&exception->snapshot, sizeof exception->snapshot);
        }
        /* No safepoint separates publication and its normal reference barrier.
         * A losing snapshot has no owner and is reclaimed by the ordinary GC. */
        scoop_thread_pop_managed_anchor(&anchor);
    }
    free(trace.bytes);
    scoop_rt_pop_native_roots(&roots);
    return exception;
}

void *scoop_rt_prepare_throwable_impl(void *object, const ScoopTypeDescriptor *trace_bytes_td,
                                      uintptr_t return_pc, uintptr_t stack_pointer,
                                      uintptr_t frame_pointer) {
    ScoopThrowable *exception = object;
    if (!scoop_stack_trace_enabled ||
        __atomic_load_n(&exception->snapshot, __ATOMIC_ACQUIRE) != NULL) {
        return exception;
    }
    return prepare_trace(exception, trace_bytes_td, return_pc, stack_pointer, frame_pointer);
}

void scoop_rt_throwable_print_stack_trace_impl(void *object, uintptr_t return_pc,
                                               uintptr_t stack_pointer, uintptr_t frame_pointer) {
    ScoopThrowable *exception = object;
    void **slots[] = {(void **)&exception};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    ScoopCallerRootFrame caller_roots;
    scoop_rt_push_caller_roots(&caller_roots, NULL, 0);
    ScoopThrowableReport report = {0};
    ScoopThreadTransition transition;
    size_t required;
    while ((required = scoop_throwable_report_copy(exception, &report)) != 0) {
        scoop_rt_enter_native_safe_impl(&transition, stack_pointer, return_pc, stack_pointer,
                                        frame_pointer);
        scoop_throwable_report_reserve(&report, required);
        scoop_rt_leave_native_safe(&transition);
        /* A concurrent first throw can publish a snapshot during the wait.
         * Re-read only the rooted object after the transition. */
    }
    scoop_rt_enter_native_safe_impl(&transition, stack_pointer, return_pc, stack_pointer,
                                    frame_pointer);
    scoop_throwable_report_render(&report, stderr);
    free(report.bytes);
    scoop_rt_leave_native_safe(&transition);
    scoop_rt_pop_caller_roots(&caller_roots);
    scoop_rt_pop_native_roots(&roots);
}
