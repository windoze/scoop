/* Public GC initialization and allocation entry classes.
 *
 * gc/heap.c owns arena storage and side metadata; gc/collector.c owns STW
 * tracing/relocation. This file only joins those components to the exported
 * ManagedEntry and NativeBorrowedEntry ABI. */
#include <stddef.h>
#include <stdint.h>

#include "scoop_rt.h"
#include "gc/gc_internal.h"
#include "managed_entries.h"
#include "thread.h"

void scoop_rt_gc_init(void) {
    scoop_gc_stackmaps_init();
    scoop_gc_register_image_roots(
        scoop_image_managed_globals, scoop_image_managed_global_count,
        scoop_image_immortal_objects, scoop_image_immortal_object_count);
    scoop_gc_heap_init();
}

void *scoop_runtime_alloc_slow_impl(const ScoopTypeDescriptor *td, size_t size,
                                    uintptr_t return_pc,
                                    uintptr_t stack_pointer,
                                    uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    scoop_thread_poll();
    void *object = scoop_gc_alloc_internal(td, size);
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size) {
    scoop_thread_native_borrowed_entry();
    return scoop_gc_alloc_internal(td, size);
}
