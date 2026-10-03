#include <string.h>

#include "gc/gc_internal.h"
#include "managed_entries.h"
#include "thread.h"
#include "value_shape.h"

const void *scoop_rt_array_clone_impl(const void *object,
                                      const ScoopTypeDescriptor *source_td,
                                      const ScoopTypeDescriptor *target_td,
                                      uintptr_t return_pc, uintptr_t stack_pointer,
                                      uintptr_t frame_pointer) {
    const ScoopTypeInstanceShapeV1 *source_shape =
        scoop_shape_require(source_td, SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1);
    const ScoopTypeInstanceShapeV1 *target_shape =
        scoop_shape_require(target_td, SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1);
    if (source_shape->inline_storage_kind != target_shape->inline_storage_kind ||
        source_shape->inline_size != target_shape->inline_size ||
        source_shape->inline_alignment != target_shape->inline_alignment) {
        scoop_shape_fatal("array clone element storage mismatch");
    }
#if defined(SCOOP_VERIFY_METADATA) && SCOOP_VERIFY_METADATA
    if (!scoop_shape_scan_equal(source_shape->inline_scan, target_shape->inline_scan,
                                0)) {
        scoop_shape_fatal("array clone element scan mismatch");
    }
#endif
    scoop_gc_heap_lock();
    if (!scoop_gc_is_object_start_locked(object)) {
        scoop_shape_fatal("array clone requires a managed object start");
    }
    if (((const ScoopObjectHeader *)object)->td != source_td) {
        scoop_shape_fatal("array clone exact source TypeDescriptor mismatch");
    }
    scoop_shape_validate_object(object, scoop_gc_object_size_locked(object));
    scoop_gc_heap_unlock();
    const ScoopArray *source = object;
    uint64_t count = source->size;
    size_t bytes = scoop_shape_allocation_size(target_td, count);
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    void **root_slots[] = {(void **)&source};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, root_slots, 1);
    scoop_thread_poll();
    ScoopArray *copy = scoop_gc_alloc_internal(target_td, bytes);
    copy->size = count;
    if (target_shape->inline_storage_kind == SCOOP_INLINE_STORAGE_INLINE_V1 &&
        count != 0) {
        memcpy((char *)copy + target_shape->inline_offset,
               (const char *)source + source_shape->inline_offset,
               (size_t)(count * target_shape->inline_stride));
    }
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_pop_managed_anchor(&anchor);
    return copy;
}
