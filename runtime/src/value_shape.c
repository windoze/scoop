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

static bool empty_inline(const ScoopTypeInstanceShapeV1 *shape) {
    return shape->inline_storage_kind == SCOOP_INLINE_STORAGE_NONE_V1 &&
           shape->inline_offset == 0 && shape->inline_size == 0 &&
           shape->inline_stride == 0 && shape->inline_alignment == 0 &&
           shape->inline_scan == NULL;
}

static void validate_inline(const ScoopTypeDescriptor *td) {
    const ScoopTypeInstanceShapeV1 *shape = &td->instance_shape;
    if (!valid_alignment(shape->inline_alignment) ||
        shape->instance_alignment !=
            (shape->inline_alignment < 8 ? 8 : shape->inline_alignment)) {
        scoop_shape_fatal("inline storage alignment disagrees with its instance");
    }
    switch (shape->inline_storage_kind) {
    case SCOOP_INLINE_STORAGE_ZERO_SIZED_V1:
        if (shape->inline_size != 0 || shape->inline_stride != 0 ||
            shape->inline_scan != NULL || td->object_scan != NULL) {
            scoop_shape_fatal("zero-sized storage has payload or reference scan");
        }
        return;
    case SCOOP_INLINE_STORAGE_INLINE_V1:
        if (shape->inline_size == 0 ||
            shape->inline_size % shape->inline_alignment != 0) {
            scoop_shape_fatal("inline storage has invalid size or alignment");
        }
        scoop_shape_scan_validate(shape->inline_scan, shape->inline_size);
        return;
    default:
        scoop_shape_fatal("invalid inline storage kind");
    }
}

void scoop_shape_validate(const ScoopTypeDescriptor *td) {
    if (td == NULL) {
        scoop_shape_fatal("operation has no TypeDescriptor");
    }
    const ScoopTypeInstanceShapeV1 *shape = &td->instance_shape;
    if (shape->instance_kind == SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1) {
        if (shape->minimum_size != 0 || shape->instance_alignment != 0 ||
            td->object_scan != NULL || !empty_inline(shape)) {
            scoop_shape_fatal("invalid abstract-reference shape");
        }
        return;
    }
    if (!valid_alignment(shape->instance_alignment) || shape->instance_alignment < 8 ||
        shape->minimum_size < 16 ||
        shape->minimum_size % shape->instance_alignment != 0) {
        scoop_shape_fatal("invalid managed instance size or alignment");
    }
    (void)checked_size(shape->minimum_size);
    switch (shape->instance_kind) {
    case SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1:
        if (!empty_inline(shape)) {
            scoop_shape_fatal("fixed object carries inline storage");
        }
        scoop_shape_scan_validate(td->object_scan, shape->minimum_size);
        return;
    case SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1:
        validate_inline(td);
        if (shape->inline_stride != 0 ||
            shape->inline_offset != scoop_shape_align(16, shape->inline_alignment) ||
            shape->minimum_size !=
                scoop_shape_align(
                    scoop_shape_add(shape->inline_offset, shape->inline_size),
                    shape->instance_alignment) ||
            !scoop_shape_scan_equal(td->object_scan, shape->inline_scan,
                                    shape->inline_offset)) {
            scoop_shape_fatal(
                "boxed payload layout or scan disagrees with its instance");
        }
        return;
    case SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1:
        if (shape->inline_storage_kind != SCOOP_INLINE_STORAGE_INLINE_V1 ||
            shape->minimum_size != 24 || shape->instance_alignment != 8 ||
            shape->inline_offset != 24 || shape->inline_size != 1 ||
            shape->inline_stride != 1 || shape->inline_alignment != 1 ||
            shape->inline_scan != NULL || td->object_scan != NULL) {
            scoop_shape_fatal("invalid inline-byte shape");
        }
        return;
    case SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1:
        validate_inline(td);
        if (shape->inline_offset != scoop_shape_align(24, shape->inline_alignment) ||
            shape->minimum_size != shape->inline_offset ||
            shape->inline_stride != shape->inline_size) {
            scoop_shape_fatal("invalid inline-array layout");
        }
        if (shape->inline_scan == NULL) {
            if (td->object_scan != NULL) {
                scoop_shape_fatal("GC-free array has an object scan");
            }
        } else {
            const uint64_t *scan = td->object_scan;
            if (scan == NULL || scan[0] != SCOOP_REFS_ARRAY || scan[1] != 16 ||
                scan[2] != shape->inline_offset || scan[3] != shape->inline_stride ||
                !scoop_shape_scan_equal((const uint64_t *)(uintptr_t)scan[4],
                                        shape->inline_scan, 0)) {
                scoop_shape_fatal("array scan disagrees with its element layout");
            }
        }
        return;
    default:
        scoop_shape_fatal("unknown managed instance kind");
    }
}

static const ScoopTypeInstanceShapeV1 *operation_shape(const ScoopTypeDescriptor *td) {
    scoop_image_require_type(td);
#if defined(SCOOP_VERIFY_METADATA) && SCOOP_VERIFY_METADATA
    scoop_shape_validate(td);
#endif
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
