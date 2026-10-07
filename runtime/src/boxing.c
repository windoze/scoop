#include <string.h>

#include "gc/gc_internal.h"
#include "managed_entries.h"
#include "thread.h"
#include "value_shape.h"

static const ScoopTypeInstanceShapeV1 *box_shape(const ScoopTypeDescriptor *td,
                                                 uint32_t storage_kind) {
    const ScoopTypeInstanceShapeV1 *shape =
        scoop_shape_require(td, SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1);
    if (shape->inline_storage_kind != storage_kind) {
        scoop_shape_fatal("boxing entry disagrees with the payload storage kind");
    }
    return shape;
}

static void require_place(const void *place, const ScoopTypeInstanceShapeV1 *shape) {
    if (place == NULL || shape->inline_alignment == 0 ||
        (uintptr_t)place % shape->inline_alignment != 0) {
        scoop_shape_fatal("boxed value place is null or misaligned");
    }
}

static void require_payload_root(const void *source, const uint64_t *scan) {
    if (scan == NULL) {
        return;
    }
    const ScoopThreadState *thread = scoop_thread_current_required();
    for (const ScoopNativeRegionRootFrame *frame = thread->native_region_roots;
         frame != NULL; frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            if (frame->entries[index].base == source &&
                frame->entries[index].scan == scan) {
                return;
            }
        }
    }
    scoop_shape_fatal("box payload root must be active before managed entry");
}

void *scoop_rt_box_zst_impl(const ScoopTypeDescriptor *td, uintptr_t return_pc,
                            uintptr_t stack_pointer, uintptr_t frame_pointer) {
    const ScoopTypeInstanceShapeV1 *shape =
        box_shape(td, SCOOP_INLINE_STORAGE_ZERO_SIZED_V1);
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_thread_poll();
    void *object = scoop_gc_alloc_internal(td, (size_t)shape->minimum_size);
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

void *scoop_rt_box_value_impl(const ScoopTypeDescriptor *td, const void *source,
                              uintptr_t return_pc, uintptr_t stack_pointer,
                              uintptr_t frame_pointer) {
    const ScoopTypeInstanceShapeV1 *shape =
        box_shape(td, SCOOP_INLINE_STORAGE_INLINE_V1);
    require_place(source, shape);
    /* This check precedes entry publication and any possible GC handshake.
     * The caller owns the stable, writable source region until return. */
    require_payload_root(source, shape->inline_scan);
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_thread_poll();
    void *object = scoop_gc_alloc_internal(td, (size_t)shape->minimum_size);
    memcpy((char *)object + shape->inline_offset, source, (size_t)shape->inline_size);
    if (shape->inline_scan != NULL) {
        scoop_rt_gc_write_barrier((char *)object + shape->inline_offset,
                                   (size_t)shape->inline_size);
    }
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

static const ScoopTypeInstanceShapeV1 *
unbox_shape(const void *object, const ScoopTypeDescriptor *expected_td,
            uint32_t storage_kind) {
    const ScoopTypeInstanceShapeV1 *shape = box_shape(expected_td, storage_kind);
    scoop_gc_heap_lock();
    if (!scoop_gc_is_object_start_locked(object)) {
        scoop_shape_fatal("unbox requires a managed object start");
    }
    if (((const ScoopObjectHeader *)object)->td != expected_td) {
        scoop_shape_fatal("unbox exact TypeDescriptor mismatch");
    }
    scoop_shape_validate_object(object, scoop_gc_object_size_locked(object));
    scoop_gc_heap_unlock();
    return shape;
}

void scoop_rt_unbox_zst(const void *object, const ScoopTypeDescriptor *expected_td) {
    (void)unbox_shape(object, expected_td, SCOOP_INLINE_STORAGE_ZERO_SIZED_V1);
}

void scoop_rt_unbox_value(const void *object, const ScoopTypeDescriptor *expected_td,
                          void *destination) {
    const ScoopTypeInstanceShapeV1 *shape =
        unbox_shape(object, expected_td, SCOOP_INLINE_STORAGE_INLINE_V1);
    require_place(destination, shape);
    memcpy(destination, (const char *)object + shape->inline_offset,
           (size_t)shape->inline_size);
}
