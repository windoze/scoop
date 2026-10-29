#include "process.h"
#include "stack_trace.h"
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "scoop_rt.h"
#include "thread/internal.h"

void scoop_process_prepare_termination(void) {
    ScoopThreadState *state = scoop_thread_current();
    if (state == NULL) {
        return;
    }
    ScoopThreadMode mode = atomic_load_explicit(&state->poll.mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_NATIVE_SAFE) {
        return;
    }
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_MANAGED_PENDING &&
        mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_thread_fatal("process termination from an invalid thread state");
    }
    /* This path never uses managed locals again. Keep published roots, pins,
     * and outer frozen segments alive until _exit ends every thread. */
    atomic_store_explicit(&state->poll.mode, SCOOP_THREAD_NATIVE_SAFE, memory_order_seq_cst);
}

static _Noreturn void terminate_process(int32_t code) {
    fflush(stdout);
    fflush(stderr);
    _exit(code);
}

_Noreturn void *scoop_rt_exit(int32_t code) {
    scoop_process_prepare_termination();
    terminate_process(code);
}

/* Keep the large capture buffer out of the entry that reads its caller PC.
 * Darwin's AArch64 stack probe may clobber LR before a return-address builtin. */
__attribute__((noinline)) static _Noreturn void report_failure(
    const char *message, uintptr_t caller_pc, int fatal) {
    ScoopTraceFrames frames;
    scoop_trace_capture_frames(&frames, caller_pc);
    scoop_process_prepare_termination();
    ScoopTraceBuffer trace = scoop_trace_complete(&frames);
    fprintf(stderr, "scoop: %s: %s\n", fatal ? "fatal" : "trap", message);
    scoop_trace_render(trace.bytes, stderr);
    free(trace.bytes);
    if (fatal)
        abort();
    terminate_process(1);
}

_Noreturn void scoop_rt_trap(const char *message) {
    report_failure(message, (uintptr_t)__builtin_return_address(0), 0);
}

_Noreturn void scoop_rt_fatal(const char *message) {
    report_failure(message, (uintptr_t)__builtin_return_address(0), 1);
}

_Noreturn void scoop_rt_allocation_overflow(void) { scoop_rt_trap("array size overflow"); }
