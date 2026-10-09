#include "scoop_rt.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    ScoopObjectHeader header;
    const ScoopArray *storage;
    int64_t count;
} List;

void m34_inspect_list(const List *list, int64_t stride) {
    assert(list->header.td->instance_shape.minimum_size == sizeof(List));
    const ScoopArray *storage = list->storage;
    const ScoopTypeInstanceShapeV1 *shape = &storage->header.td->instance_shape;
    assert(shape->instance_kind == SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1);
    assert(shape->inline_stride == (uint64_t)stride);
    assert(shape->inline_size == (uint64_t)stride);
    assert(list->count >= 0 && (uint64_t)list->count <= storage->size);
    const unsigned char *bytes = (const unsigned char *)storage + shape->inline_offset;
    for (uint64_t index = (uint64_t)list->count * shape->inline_stride;
         index < storage->size * shape->inline_stride; ++index) {
        assert(bytes[index] == 0);
    }
}

int64_t m34_minor_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.minor_collections;
}

bool m34_moving_stress(void) {
    const char *value = getenv("SCOOP_GC_STRESS_MOVE");
    return value != NULL && strcmp(value, "1") == 0;
}
