#include <string.h>

#include "scoop_rt.h"
#include "value_shape.h"

extern const ScoopTypeDescriptor scoop_td_String;

const ScoopString *scoop_rt_string_join_parts(const ScoopArray *storage,
                                              int64_t part_count) {
    if (part_count < 0 || (uint64_t)part_count > storage->size) {
        scoop_shape_fatal("String parts prefix out of bounds");
    }
    const ScoopTypeInstanceShapeV1 *shape =
        scoop_shape_require(storage->header.td, SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1);
    const ScoopArray *source = storage;
    void **slots[] = {(void **)&source};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    const ScoopString *const *parts =
        (const ScoopString *const *)((const char *)source + shape->inline_offset);
    uint64_t length = 0;
    for (int64_t index = 0; index < part_count; ++index) {
        if (parts[index] == NULL) {
            scoop_shape_fatal("String parts prefix contains None");
        }
        length = scoop_shape_add(length, parts[index]->len);
    }
    if (part_count == 1) {
        const ScoopString *result = parts[0];
        scoop_rt_pop_native_roots(&roots);
        return result;
    }
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, length));
    result->len = length;
    parts = (const ScoopString *const *)((const char *)source + shape->inline_offset);
    uint64_t cursor = 0;
    for (int64_t index = 0; index < part_count; ++index) {
        const ScoopString *part = parts[index];
        memcpy(result->data + cursor, part->data, (size_t)part->len);
        cursor += part->len;
    }
    scoop_rt_pop_native_roots(&roots);
    return result;
}
