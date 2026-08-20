#include <stdlib.h>
#include <string.h>

#include "../value_shape.h"
#include "internal.h"

typedef struct StaticScanFrame {
    const uint64_t *program;
    uint64_t offset;
    uint64_t extent;
    uint64_t index;
} StaticScanFrame;

typedef struct StaticLeaves {
    uint64_t *offsets;
    size_t count, capacity;
} StaticLeaves;

static void *grow(void *data, size_t *capacity, size_t size) {
    size_t next = *capacity == 0 ? 16 : *capacity * 2;
    if (next < *capacity || next > SIZE_MAX / size) {
        scoop_metadata_fatal(NULL, "static scan index size overflow");
    }
    void *result = realloc(data, next * size);
    if (result == NULL) {
        scoop_metadata_fatal(NULL, "cannot allocate static scan index");
    }
    *capacity = next;
    return result;
}

static int offset_order(const void *left, const void *right) {
    uint64_t a = *(const uint64_t *)left, b = *(const uint64_t *)right;
    return (a > b) - (a < b);
}

static StaticLeaves
collect_leaves(const ScoopMetadataCheck *check,
               const ScoopStaticStorageDescriptorV1 *storage) {
    StaticLeaves leaves = {0};
    if (storage->scan_kind == SCOOP_STATIC_SCAN_NONE_V1) {
        return leaves;
    }
    size_t count = 1, capacity = 1;
    StaticScanFrame *frames = scoop_metadata_allocate(capacity, sizeof *frames);
    frames[0] =
        (StaticScanFrame){storage->scan_program, 0, storage->byte_size, 0};
    while (count != 0) {
        StaticScanFrame *frame = &frames[count - 1];
        const uint64_t *scan = frame->program;
        StaticScanFrame child;
        if (scan[0] == SCOOP_REFS_ARRAY) {
            uint64_t length;
            memcpy(&length,
                   (const uint8_t *)storage->writable_base + frame->offset +
                       scan[1],
                   8);
            if (length > (frame->extent - scan[2]) / scan[3]) {
                scoop_metadata_fatal(
                    check, "static array scan length exceeds storage");
            }
            if (frame->index == length) {
                count--;
                continue;
            }
            child = (StaticScanFrame){
                (const uint64_t *)(uintptr_t)scan[4],
                frame->offset + scan[2] + frame->index++ * scan[3], scan[3], 0};
        } else if (scan[0] == SCOOP_REFS_SEQUENCE) {
            if (frame->index == scan[1]) {
                count--;
                continue;
            }
            child = (StaticScanFrame){
                (const uint64_t *)(uintptr_t)scan[2 + frame->index++],
                frame->offset, frame->extent, 0};
        } else {
            for (uint64_t index = 0; index < scan[0]; index++) {
                uint64_t offset = frame->offset + scan[index + 1];
                if (offset % 8 != 0 ||
                    ((uintptr_t)storage->writable_base + offset) % 8 != 0) {
                    scoop_metadata_fatal(check, "unaligned static scan leaf");
                }
                if (leaves.count == leaves.capacity) {
                    leaves.offsets = grow(leaves.offsets, &leaves.capacity,
                                          sizeof *leaves.offsets);
                }
                leaves.offsets[leaves.count++] = offset;
            }
            count--;
            continue;
        }
        if (count == capacity) {
            frames = grow(frames, &capacity, sizeof *frames);
        }
        frames[count++] = child;
    }
    free(frames);
    qsort(leaves.offsets, leaves.count, sizeof *leaves.offsets, offset_order);
    for (size_t index = 1; index < leaves.count; index++) {
        if (leaves.offsets[index] == leaves.offsets[index - 1]) {
            scoop_metadata_fatal(check, "duplicate static scan leaf");
        }
    }
    return leaves;
}

void scoop_image_initial_values(const ScoopImageRegistry *registry,
                                size_t index) {
    ScoopMetadataCheck check =
        scoop_record_check(registry, SCOOP_RECORD_STORAGE, index);
    const ScoopStaticStorageDescriptorV1 *storage =
        registry->tables[SCOOP_RECORD_STORAGE].entries[index].record;
    StaticLeaves leaves = collect_leaves(&check, storage);
    size_t relocation = 0;
    for (size_t leaf = 0; leaf < leaves.count; leaf++) {
        uint64_t offset = leaves.offsets[leaf];
        const void *actual;
        memcpy(&actual, (const uint8_t *)storage->writable_base + offset,
               sizeof actual);
        if (relocation < storage->initial_relocation_count &&
            storage->initial_relocations[relocation].pointer_offset <= offset) {
            const ScoopStaticImmortalRelocationV1 *entry =
                &storage->initial_relocations[relocation++];
            if (entry->pointer_offset != offset ||
                scoop_record_by_address(registry, SCOOP_RECORD_IMMORTAL,
                                        entry->target) == NULL) {
                scoop_metadata_fatal(
                    &check,
                    "relocation offset or unregistered immortal target");
            }
            if (actual != entry->target->object_start) {
                scoop_metadata_fatal(
                    &check,
                    "initial reference is not the exact immortal start");
            }
        } else if (actual != NULL) {
            scoop_metadata_fatal(
                &check, "nonzero initial reference has no immortal relocation");
        }
    }
    if (relocation != storage->initial_relocation_count) {
        scoop_metadata_fatal(&check,
                             "relocations are not ordered unique scan leaves");
    }
    free(leaves.offsets);
}
