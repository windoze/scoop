#include <string.h>

#include "gc.h"
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

static const ScoopTypeInstanceShapeV1 *
unbox_shape(const void *object, const ScoopTypeDescriptor *expected_td, uint32_t storage_kind) {
    const ScoopTypeInstanceShapeV1 *shape = box_shape(expected_td, storage_kind);
    scoop_gc_check_unbox(object, expected_td, storage_kind);
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
    memcpy(destination, (const char *)object + shape->inline_offset, (size_t)shape->inline_size);
}
