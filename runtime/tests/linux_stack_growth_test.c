#include <assert.h>
#include <stdio.h>

#include "../src/thread.h"

extern const ScoopThreadVmOps scoop_linux_thread_vm_ops;

ScoopPlatformStackBounds scoop_platform_stack_bounds(void) {
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    assert(scoop_linux_thread_vm_ops.stack_bounds(&bounds, &error));
    return bounds;
}

static unsigned refreshed;
static volatile unsigned touched;

static void check_refresh(uintptr_t previous_low, uintptr_t address) {
    ScoopThreadState *state = scoop_thread_current_required();
    assert((uintptr_t)state->stack_low <= address);
    assert(address < (uintptr_t)state->stack_high);
    if ((uintptr_t)state->stack_low < previous_low) {
        ++refreshed;
    }
}

__attribute__((noinline)) static void descend(unsigned remaining,
                                              void (*action)(void)) {
    volatile unsigned char padding[8192];
    padding[0] = (unsigned char)remaining;
    padding[sizeof padding - 1] = (unsigned char)(remaining + 1);
    if (remaining == 0) {
        action();
    } else {
        descend(remaining - 1, action);
    }
    touched += padding[0] + padding[sizeof padding - 1];
}

static void callback_entry(void) {
    char marker;
    ScoopThreadState *state = scoop_thread_current_required();
    uintptr_t previous_low = (uintptr_t)state->stack_low;
    ScoopCallbackThreadEntry callback = {0};
    scoop_thread_enter_callback(&callback, &marker);
    check_refresh(previous_low, (uintptr_t)&marker);
    scoop_thread_leave_callback(&callback);
}

static void native_entry(void) {
    char marker;
    ScoopThreadState *state = scoop_thread_current_required();
    uintptr_t previous_low = (uintptr_t)state->stack_low;
    ScoopCallerRootFrame roots;
    scoop_rt_push_caller_roots(&roots, NULL, 0);
    ScoopThreadTransition transition = {0};
    scoop_rt_enter_native_safe_impl(&transition, (uintptr_t)&marker, 1,
                                    (uintptr_t)&marker,
                                    (uintptr_t)__builtin_frame_address(0));
    check_refresh(previous_low, (uintptr_t)&marker);
    descend(128, callback_entry);
    scoop_rt_leave_native_safe(&transition);
    scoop_rt_pop_caller_roots(&roots);
}

static void managed_anchor(void) {
    char marker;
    ScoopThreadState *state = scoop_thread_current_required();
    uintptr_t previous_low = (uintptr_t)state->stack_low;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 1, (uintptr_t)&marker,
                                     (uintptr_t)__builtin_frame_address(0));
    check_refresh(previous_low, (uintptr_t)&marker);
    scoop_thread_pop_managed_anchor(&anchor);
    descend(128, native_entry);
}

static void managed_entry(void) {
    char marker;
    ScoopThreadState *state = scoop_thread_current_required();
    uintptr_t previous_low = (uintptr_t)state->stack_low;
    scoop_thread_enter_managed(&marker);
    check_refresh(previous_low, (uintptr_t)&marker);
    descend(128, managed_anchor);
    scoop_thread_leave_managed();
}

int main(void) {
    scoop_thread_runtime_init();
    scoop_thread_attach_main();
    descend(128, managed_entry);
    assert(touched != 0);
#ifndef __GLIBC__
    /* musl reports the currently mapped main stack, which grows on demand. */
    assert(refreshed == 4);
#endif
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("Linux stack growth across managed, native and callback entries "
         "passed");
}
