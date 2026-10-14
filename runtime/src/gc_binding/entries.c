/* Public GC initialization and allocation entry classes.
 *
 * The GC core owns region storage and side metadata, STW
 * tracing/relocation. This file only joins those components to the exported
 * ManagedEntry and NativeBorrowedEntry ABI. */
#include <stddef.h>
#include <stdint.h>

#include "../gc/gc_internal.h"
#include "internal.h"
#include "../managed_entries.h"
#include "scoop_rt.h"
#include "../thread.h"

void scoop_rt_gc_init(const ScoopImageRegistry *registry) {
    scoop_gc_stackmaps_init(&registry->stackmaps);
    scoop_gc_heap_init();
}

void *scoop_runtime_alloc_slow_impl(const ScoopTypeDescriptor *td, size_t size, uintptr_t return_pc,
                                    uintptr_t stack_pointer, uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_thread_poll();
    void *object = scoop_gc_alloc_internal(td, size);
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size) {
    scoop_thread_native_borrowed_entry();
    return scoop_gc_alloc_internal(td, size);
}

void scoop_rt_gc_collect_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                              uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_gc_collect_internal();
    scoop_thread_pop_managed_anchor(&anchor);
}

void scoop_runtime_gc_collect(void) {
    scoop_thread_native_borrowed_entry();
    scoop_gc_collect_internal();
}

void scoop_rt_safepoint_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                             uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_safepoint_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_thread_poll();
    scoop_thread_pop_managed_anchor(&anchor);
}
