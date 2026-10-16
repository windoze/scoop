#include <stdio.h>
#include <stdlib.h>

#include "image/registry.h"
#include "value_shape.h"

_Noreturn void scoop_shape_fatal(const char *message) {
    fprintf(stderr, "scoop runtime shape: %s\n", message);
    abort();
}

uint64_t scoop_shape_add(uint64_t left, uint64_t right) {
    if (right > UINT64_MAX - left) {
        scoop_shape_fatal("allocation size overflow");
    }
    return left + right;
}

static bool valid_alignment(uint64_t alignment) {
    return alignment != 0 && (alignment & (alignment - 1)) == 0 &&
           alignment <= SCOOP_MAXIMUM_MANAGED_ALIGNMENT;
}

uint64_t scoop_shape_align(uint64_t size, uint64_t alignment) {
    if (!valid_alignment(alignment)) {
        scoop_shape_fatal("invalid managed alignment");
    }
    return scoop_shape_add(size, alignment - 1) & ~(alignment - 1);
}

static size_t checked_size(uint64_t size) {
    if (size > SIZE_MAX || size > SCOOP_MAXIMUM_MANAGED_OBJECT_SIZE) {
        scoop_shape_fatal("allocation exceeds the target object-size limit");
    }
    return (size_t)size;
}

static const ScoopTypeInstanceShapeV1 *operation_shape(const ScoopTypeDescriptor *td) {
    scoop_image_require_type(td);
    return &td->instance_shape;
}

const ScoopTypeInstanceShapeV1 *scoop_shape_require(const ScoopTypeDescriptor *td,
                                                    uint32_t kind) {
    const ScoopTypeInstanceShapeV1 *shape = operation_shape(td);
    if (shape->instance_kind != kind) {
        scoop_shape_fatal("operation received the wrong TypeDescriptor shape");
    }
    return shape;
}

static size_t allocation_size(const ScoopTypeInstanceShapeV1 *shape, uint64_t count) {
    if (shape->instance_kind != SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1 &&
        shape->instance_kind != SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1) {
        scoop_shape_fatal("count allocation requires variable inline storage");
    }
    if (count > INT64_MAX) {
        scoop_shape_fatal("physical count exceeds INT64_MAX");
    }
    if (shape->inline_storage_kind == SCOOP_INLINE_STORAGE_ZERO_SIZED_V1) {
        return checked_size(shape->minimum_size);
    }
    if (count != 0 && shape->inline_stride > UINT64_MAX / count) {
        scoop_shape_fatal("inline payload size overflow");
    }
    return checked_size(scoop_shape_align(
        scoop_shape_add(shape->inline_offset, count * shape->inline_stride),
        shape->instance_alignment));
}

size_t scoop_shape_allocation_size(const ScoopTypeDescriptor *td, uint64_t count) {
    return allocation_size(operation_shape(td), count);
}

size_t scoop_shape_normalize_allocation(const ScoopTypeDescriptor *td,
                                        size_t requested) {
    const ScoopTypeInstanceShapeV1 *shape = operation_shape(td);
    if (shape->instance_kind == SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1) {
        scoop_shape_fatal("abstract reference is not allocatable");
    }
    size_t normalized =
        checked_size(scoop_shape_align(requested, shape->instance_alignment));
    bool fixed = shape->instance_kind == SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1 ||
                 shape->instance_kind == SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1 ||
                 shape->inline_storage_kind == SCOOP_INLINE_STORAGE_ZERO_SIZED_V1;
    if (normalized < shape->minimum_size ||
        (fixed && normalized != shape->minimum_size)) {
        scoop_shape_fatal("allocation size disagrees with its TypeDescriptor");
    }
    return normalized;
}

void scoop_shape_validate_object_size(const void *object, size_t object_size,
                                      const ScoopTypeDescriptor *td) {
    if (object == NULL || object_size < sizeof(ScoopObjectHeader)) {
        scoop_shape_fatal("object range cannot hold its header");
    }
    const ScoopTypeInstanceShapeV1 *shape = &td->instance_shape;
    if (object_size < shape->minimum_size) {
        scoop_shape_fatal("object side metadata is smaller than its header shape");
    }
    size_t expected = (size_t)shape->minimum_size;
    if (shape->instance_kind == SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1 ||
        shape->instance_kind == SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1) {
        expected = allocation_size(shape, ((const ScoopArray *)object)->size);
    }
    if (shape->instance_kind == SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1 ||
        object_size != expected || !valid_alignment(shape->instance_alignment) ||
        (uintptr_t)object % shape->instance_alignment != 0) {
        scoop_shape_fatal("object count, alignment or side-metadata size is invalid");
    }
}

void scoop_shape_validate_object(const void *object, size_t object_size) {
    if (object == NULL || object_size < sizeof(ScoopObjectHeader)) {
        scoop_shape_fatal("object range cannot hold its header");
    }
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    (void)operation_shape(td);
    scoop_shape_validate_object_size(object, object_size, td);
}
