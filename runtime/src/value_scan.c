#include <stdlib.h>

#include "value_shape.h"

/* The metadata is immutable. Revisit a shared node only when its translation
 * or storage extent changes; active nodes detect cycles independently of those
 * values. The explicit traversal stack does not impose a semantic depth limit. */
typedef struct ScanVisit {
    const uint64_t *left;
    const uint64_t *right;
    uint64_t translation;
    uint64_t extent;
    uint64_t next_child;
    size_t parent;
    bool complete;
} ScanVisit;

typedef struct ScanTraversal {
    ScanVisit *visits;
    size_t count;
    size_t capacity;
    size_t current;
    bool check_extent;
} ScanTraversal;

static void check_count(uint64_t count, size_t prefix) {
    if (count > SIZE_MAX / sizeof(uint64_t) - prefix) {
        scoop_shape_fatal("scan storage size overflows size_t");
    }
}

static bool check_node(const ScanVisit *visit, bool check_extent) {
    const uint64_t *left = visit->left;
    const uint64_t *right = visit->right;
    uint64_t tag = right[0];
    if (left[0] != tag) {
        return false;
    }
    if (tag == SCOOP_REFS_ARRAY) {
        if (right[3] == 0 || left[3] == 0 || right[4] == 0 || left[4] == 0 ||
            right[1] % 8 != 0 || left[1] % 8 != 0) {
            scoop_shape_fatal("invalid array scan");
        }
        if (check_extent && (visit->extent < 8 || right[1] > visit->extent - 8 ||
                             right[2] > visit->extent)) {
            scoop_shape_fatal("array scan header exceeds its storage");
        }
        return left[1] == scoop_shape_add(right[1], visit->translation) &&
               left[2] == scoop_shape_add(right[2], visit->translation) &&
               left[3] == right[3];
    }
    if (tag == SCOOP_REFS_SEQUENCE) {
        uint64_t count = right[1];
        if (count < 2 || left[1] < 2) {
            scoop_shape_fatal("invalid sequence scan");
        }
        if (left[1] != count) {
            return false;
        }
        check_count(count, 2);
        unsigned left_references = 0;
        unsigned right_references = 0;
        for (uint64_t index = 0; index < count; index++) {
            const uint64_t *child = (const uint64_t *)(uintptr_t)right[2 + index];
            const uint64_t *other = (const uint64_t *)(uintptr_t)left[2 + index];
            if (child == NULL || other == NULL || child[0] == SCOOP_REFS_SEQUENCE ||
                other[0] == SCOOP_REFS_SEQUENCE ||
                (child[0] < SCOOP_REFS_SEQUENCE && ++right_references > 1) ||
                (other[0] < SCOOP_REFS_SEQUENCE && ++left_references > 1)) {
                scoop_shape_fatal("non-normalized sequence scan");
            }
        }
        return true;
    }
    if (tag == 0) {
        scoop_shape_fatal("invalid empty reference scan");
    }
    check_count(tag, 1);
    for (uint64_t index = 0; index < tag; index++) {
        uint64_t offset = right[index + 1];
        uint64_t other = left[index + 1];
        if (offset % 8 != 0 || other % 8 != 0 ||
            (index != 0 && (offset <= right[index] || other <= left[index]))) {
            scoop_shape_fatal("reference scan offsets are not canonical");
        }
        if (check_extent && (visit->extent < 8 || offset > visit->extent - 8)) {
            scoop_shape_fatal("reference scan exceeds its storage");
        }
        if (other != scoop_shape_add(offset, visit->translation)) {
            return false;
        }
    }
    return true;
}

static bool enter(ScanTraversal *walk, const uint64_t *left, const uint64_t *right,
                  uint64_t translation, uint64_t extent) {
    if (left == NULL || right == NULL) {
        return left == right;
    }
    for (size_t index = 0; index < walk->count; index++) {
        const ScanVisit *visit = &walk->visits[index];
        if (!visit->complete && (visit->left == left || visit->right == right)) {
            scoop_shape_fatal("cyclic reference scan");
        }
        if (visit->complete && visit->left == left && visit->right == right &&
            visit->translation == translation && visit->extent == extent) {
            return true;
        }
    }
    ScanVisit visit = {.left = left,
                       .right = right,
                       .translation = translation,
                       .extent = extent,
                       .parent = walk->current};
    if (!check_node(&visit, walk->check_extent)) {
        return false;
    }
    if (walk->count == walk->capacity) {
        if (walk->capacity > SIZE_MAX / 2 / sizeof(ScanVisit)) {
            scoop_shape_fatal("scan traversal storage overflows size_t");
        }
        size_t capacity = walk->capacity == 0 ? 16 : walk->capacity * 2;
        ScanVisit *visits = realloc(walk->visits, capacity * sizeof *visits);
        if (visits == NULL) {
            scoop_shape_fatal("cannot allocate scan traversal storage");
        }
        walk->visits = visits;
        walk->capacity = capacity;
    }
    walk->current = walk->count;
    walk->visits[walk->count++] = visit;
    return true;
}

static bool traverse(const uint64_t *left, const uint64_t *right, uint64_t translation,
                     uint64_t extent, bool check_extent) {
    ScanTraversal walk = {.current = SIZE_MAX, .check_extent = check_extent};
    bool equal = enter(&walk, left, right, translation, extent);
    while (equal && walk.current != SIZE_MAX) {
        ScanVisit *visit = &walk.visits[walk.current];
        uint64_t tag = visit->right[0];
        const uint64_t *child;
        const uint64_t *other;
        if (tag == SCOOP_REFS_ARRAY && visit->next_child == 0) {
            visit->next_child++;
            child = (const uint64_t *)(uintptr_t)visit->right[4];
            other = (const uint64_t *)(uintptr_t)visit->left[4];
            translation = 0;
            extent = check_extent ? visit->right[3] : 0;
        } else if (tag == SCOOP_REFS_SEQUENCE && visit->next_child < visit->right[1]) {
            uint64_t index = 2 + visit->next_child++;
            child = (const uint64_t *)(uintptr_t)visit->right[index];
            other = (const uint64_t *)(uintptr_t)visit->left[index];
            translation = visit->translation;
            extent = visit->extent;
        } else {
            visit->complete = true;
            walk.current = visit->parent;
            continue;
        }
        equal = enter(&walk, other, child, translation, extent);
    }
    free(walk.visits);
    return equal;
}

bool scoop_shape_scan_equal(const uint64_t *left, const uint64_t *right,
                            uint64_t translation) {
    return traverse(left, right, translation, 0, false);
}

void scoop_shape_scan_validate(const uint64_t *scan, uint64_t extent) {
    (void)traverse(scan, scan, 0, extent, true);
}
