#include <stdatomic.h>
#include <stdint.h>

#include "../thread.h"
#include "gc_internal.h"
#include "scoop_rt.h"

void scoop_rt_push_native_roots(ScoopNativeRootFrame *frame, void ***slots,
                                uint64_t count) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&thread->poll.mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_MANAGED)
        scoop_thread_require_managed();
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_gc_roots_fatal(
            "native roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || (count != 0 && slots == NULL)) {
        scoop_gc_roots_fatal("invalid native root frame");
    }
    for (ScoopNativeRootFrame *active = thread->native_roots; active != NULL;
         active = active->previous) {
        if (active == frame) {
            scoop_gc_roots_fatal("native root frame is already active");
        }
    }
    for (uint64_t index = 0; index < count; index++) {
        if (slots[index] == NULL) {
            scoop_gc_roots_fatal("native root frame contains a null slot address");
        }
    }
    frame->previous = thread->native_roots;
    frame->slots = slots;
    frame->count = count;
    thread->native_roots = frame;
}

void scoop_rt_pop_native_roots(ScoopNativeRootFrame *frame) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&thread->poll.mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_MANAGED)
        scoop_thread_require_managed();
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_gc_roots_fatal(
            "native roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || thread->native_roots != frame) {
        scoop_gc_roots_fatal("native root frames must be popped in LIFO order");
    }
    thread->native_roots = frame->previous;
    frame->previous = NULL;
    frame->slots = NULL;
    frame->count = 0;
}

void scoop_rt_push_native_region_roots(ScoopNativeRegionRootFrame *frame,
                                       ScoopNativeRegionRootEntry *entries,
                                       uint64_t count) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&thread->poll.mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_MANAGED)
        scoop_thread_require_managed();
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_gc_roots_fatal(
            "native region roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || count == 0 || entries == NULL) {
        scoop_gc_roots_fatal("invalid native region root frame");
    }
    for (uint64_t index = 0; index < count; index++) {
        if (entries[index].base == NULL || entries[index].scan == NULL) {
            scoop_gc_roots_fatal("invalid native region root entry");
        }
    }
    for (ScoopNativeRegionRootFrame *active = thread->native_region_roots;
         active != NULL; active = active->previous) {
        if (active == frame) {
            scoop_gc_roots_fatal("native region root frame is already active");
        }
    }
    frame->previous = thread->native_region_roots;
    frame->entries = entries;
    frame->count = count;
    thread->native_region_roots = frame;
}

void scoop_rt_pop_native_region_roots(ScoopNativeRegionRootFrame *frame) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&thread->poll.mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_MANAGED)
        scoop_thread_require_managed();
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_gc_roots_fatal(
            "native region roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || thread->native_region_roots != frame) {
        scoop_gc_roots_fatal("native region root frames must be popped in LIFO order");
    }
    thread->native_region_roots = frame->previous;
    frame->previous = NULL;
    frame->entries = NULL;
    frame->count = 0;
}

uint64_t scoop_rt_gc_debug_native_root_count(void) {
    ScoopThreadState *thread = scoop_thread_current();
    if (thread == NULL) {
        return 0;
    }
    uint64_t count = 0;
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        count += frame->count;
    }
    return count;
}
