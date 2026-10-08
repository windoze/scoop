#include <stdio.h>
#include <unistd.h>

#include "scoop_rt.h"
#include "thread/internal.h"

static void prepare_termination(void) {
    ScoopThreadState *state = scoop_thread_current();
    if (state == NULL) {
        return;
    }
    ScoopThreadMode mode = atomic_load_explicit(&state->mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_NATIVE_SAFE) {
        return;
    }
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_MANAGED_PENDING &&
        mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_thread_fatal("process termination from an invalid thread state");
    }
    /* This path never uses managed locals again. Keep published roots, pins,
     * and outer frozen segments alive until _exit ends every thread. */
    atomic_store_explicit(&state->mode, SCOOP_THREAD_NATIVE_SAFE, memory_order_seq_cst);
}

static _Noreturn void terminate_process(int32_t code) {
    fflush(stdout);
    fflush(stderr);
    _exit(code);
}

_Noreturn void *scoop_rt_exit(int32_t code) {
    prepare_termination();
    terminate_process(code);
}

_Noreturn void scoop_rt_trap(const char *message) {
    prepare_termination();
    fprintf(stderr, "scoop: trap: %s\n", message);
    terminate_process(1);
}

_Noreturn void scoop_rt_allocation_overflow(void) { scoop_rt_trap("array size overflow"); }
