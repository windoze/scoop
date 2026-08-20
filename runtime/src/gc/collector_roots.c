/* Root publication is shared by minor and full collection. */
#include <stdatomic.h>

#include "../thread.h"
#include "gc_internal.h"
#include "heap_internal.h"

static void scan_thread(const ScoopThreadState *thread, ScoopGcRootVisitor visitor) {
    visitor.visit_slot((void **)&thread->current_task_context, visitor.context);
    for (ScoopPinFrame *frame = thread->pin_frames; frame != NULL; frame = frame->previous) {
        visitor.visit_slot(&frame->object, visitor.context);
    }
    ScoopThreadMode mode = atomic_load_explicit(&thread->poll.mode, memory_order_acquire);
    bool pending = mode == SCOOP_THREAD_MANAGED_PENDING;
    if (mode == SCOOP_THREAD_PARKED || mode == SCOOP_THREAD_COLLECTOR) {
        pending = thread->parked_from == SCOOP_THREAD_MANAGED_PENDING;
        if (thread->parked_from == SCOOP_THREAD_MANAGED) {
            scoop_gc_visit_managed_stack(thread, visitor);
        } else if (!pending && thread->parked_from != SCOOP_THREAD_NATIVE_BORROWED) {
            heap_fatal("parked thread has an invalid source mode");
        }
    } else if (mode != SCOOP_THREAD_NATIVE_SAFE && mode != SCOOP_THREAD_NATIVE_SAFE_RETURNING &&
               !pending) {
        heap_fatal("collector observed a non-quiescent thread");
    }
    if (pending && (thread->managed_anchor != NULL || thread->managed_stack_boundary == NULL)) {
        heap_fatal("pending gateway has an invalid empty segment");
    }
    for (ScoopCallerRootFrame *frame = thread->caller_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visitor.visit_region(frame->entries[index].base, frame->entries[index].scan,
                                 visitor.context);
        }
    }
    for (ScoopCompilerRootFrame *frame = thread->compiler_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visitor.visit_region(frame->entries[index].base, frame->entries[index].scan,
                                 visitor.context);
        }
    }
    for (ScoopNativeRegionRootFrame *frame = thread->native_region_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visitor.visit_region(frame->entries[index].base, frame->entries[index].scan,
                                 visitor.context);
        }
    }
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visitor.visit_slot(frame->slots[index], visitor.context);
        }
    }
    for (const ScoopThreadTransition *transition = thread->current_transition; transition != NULL;
         transition = transition->previous) {
        if (transition->caller_roots == NULL || transition->managed_return_pc == 0 ||
            transition->managed_stack_pointer == 0 || transition->managed_frame_pointer == 0 ||
            transition->managed_stack_high == 0) {
            heap_fatal("native transition has no exact frozen-segment publication");
        }
        scoop_gc_visit_managed_segment(
            thread, transition->managed_return_pc, transition->managed_stack_pointer,
            transition->managed_frame_pointer, transition->managed_stack_high, visitor);
    }
}

void scoop_gc_scan_roots(ScoopGcRootVisitor visitor) {
    scoop_gc_visit_roots_locked(visitor);
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head(); thread != NULL;
         thread = thread->registry_next) {
        scan_thread(thread, visitor);
    }
}
