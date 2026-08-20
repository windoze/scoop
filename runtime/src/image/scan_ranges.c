#include <stdlib.h>

#include "../value_shape.h"
#include "internal.h"

typedef struct ScanRange {
    const uint64_t *program;
    uint64_t next_child;
    uint64_t child_count;
    size_t child_offset;
    size_t parent;
    bool complete;
} ScanRange;

struct ScoopScanRanges {
    ScanRange *nodes;
    size_t count;
    size_t capacity;
};

static size_t enter(struct ScoopScanRanges *ranges, const ScoopMetadataCheck *check,
                    const uint64_t *program, size_t parent) {
    for (size_t index = 0; index < ranges->count; index++) {
        if (ranges->nodes[index].program == program) {
            if (!ranges->nodes[index].complete) {
                scoop_metadata_fatal(check, "cyclic scan child");
            }
            return parent;
        }
    }
    scoop_metadata_readonly(check, program, 1, 8, 8, "scan header range");
    ScanRange node = {.program = program, .parent = parent};
    uint64_t tag = program[0];
    uint64_t words;
    if (tag == SCOOP_REFS_ARRAY) {
        words = 5;
        node.child_count = 1;
        node.child_offset = 4;
    } else if (tag == SCOOP_REFS_SEQUENCE) {
        scoop_metadata_readonly(check, program, 2, 8, 8, "scan sequence header range");
        if (program[1] < 2 || program[1] > UINT64_MAX - 2) {
            scoop_metadata_fatal(check, "scan sequence count");
        }
        words = 2 + program[1];
        node.child_count = program[1];
        node.child_offset = 2;
    } else {
        words = 1 + tag;
    }
    scoop_metadata_readonly(check, program, words, 8, 8, "scan program range");
    if (ranges->count == ranges->capacity) {
        size_t capacity = ranges->capacity == 0 ? 16 : ranges->capacity * 2;
        if (capacity < ranges->capacity ||
            capacity > SIZE_MAX / sizeof *ranges->nodes) {
            scoop_metadata_fatal(check, "scan index size overflow");
        }
        ScanRange *nodes = realloc(ranges->nodes, capacity * sizeof *nodes);
        if (nodes == NULL) {
            scoop_metadata_fatal(check, "cannot allocate scan index");
        }
        ranges->nodes = nodes;
        ranges->capacity = capacity;
    }
    size_t index = ranges->count++;
    ranges->nodes[index] = node;
    return index;
}

void scoop_metadata_scan(ScoopImageRegistry *registry, const ScoopMetadataCheck *check,
                         const uint64_t *scan) {
    if (scan == NULL) {
        return;
    }
    if (registry->scan_ranges == NULL) {
        registry->scan_ranges =
            scoop_metadata_allocate(1, sizeof *registry->scan_ranges);
    }
    struct ScoopScanRanges *ranges = registry->scan_ranges;
    size_t current = enter(ranges, check, scan, SIZE_MAX);
    while (current != SIZE_MAX) {
        ScanRange *node = &ranges->nodes[current];
        if (node->next_child == node->child_count) {
            node->complete = true;
            current = node->parent;
        } else {
            const uint64_t *child =
                (const uint64_t *)(uintptr_t)
                    node->program[node->child_offset + node->next_child++];
            current = enter(ranges, check, child, current);
        }
    }
}

void scoop_metadata_scan_dispose(ScoopImageRegistry *registry) {
    if (registry->scan_ranges != NULL) {
        free(registry->scan_ranges->nodes);
        free(registry->scan_ranges);
        registry->scan_ranges = NULL;
    }
}
