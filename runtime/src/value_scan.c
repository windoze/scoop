#include "value_shape.h"

typedef struct ScanComparisonBudget {
    uint64_t nodes;
    uint64_t bytes;
} ScanComparisonBudget;

static void consume(ScanComparisonBudget *budget, uint64_t bytes) {
    if (++budget->nodes > UINT64_C(1048576) ||
        bytes > UINT64_C(16777216) - budget->bytes) {
        scoop_shape_fatal("scan comparison exceeds its expansion budget");
    }
    budget->bytes += bytes;
}

static bool compare_scan(const uint64_t *left, const uint64_t *right,
                         uint64_t translation, uint64_t depth,
                         ScanComparisonBudget *budget) {
    if (depth > 64) {
        scoop_shape_fatal("scan comparison exceeds its depth limit");
    }
    if (left == NULL || right == NULL) {
        consume(budget, 4);
        return left == right;
    }
    if (left[0] != right[0]) {
        return false;
    }
    uint64_t tag = right[0];
    if (tag == SCOOP_REFS_ARRAY) {
        consume(budget, 28);
        if (right[3] == 0 || right[4] == 0 || left[4] == 0 || right[1] % 8 != 0 ||
            left[1] % 8 != 0) {
            scoop_shape_fatal("invalid array scan");
        }
        return left[1] == scoop_shape_add(right[1], translation) &&
               left[2] == scoop_shape_add(right[2], translation) &&
               left[3] == right[3] &&
               compare_scan((const uint64_t *)(uintptr_t)left[4],
                            (const uint64_t *)(uintptr_t)right[4], 0, depth + 1,
                            budget);
    }
    if (tag == SCOOP_REFS_SEQUENCE) {
        uint64_t count = right[1];
        if (count < 2 || count > UINT64_C(65536) || left[1] != count) {
            scoop_shape_fatal("invalid sequence scan");
        }
        consume(budget, 12);
        unsigned references = 0;
        for (uint64_t index = 0; index < count; index++) {
            const uint64_t *child = (const uint64_t *)(uintptr_t)right[2 + index];
            const uint64_t *other = (const uint64_t *)(uintptr_t)left[2 + index];
            if (child == NULL || other == NULL || child[0] == SCOOP_REFS_SEQUENCE ||
                other[0] == SCOOP_REFS_SEQUENCE ||
                (child[0] < SCOOP_REFS_SEQUENCE && ++references > 1)) {
                scoop_shape_fatal("non-normalized sequence scan");
            }
            if (!compare_scan(other, child, translation, depth + 1, budget)) {
                return false;
            }
        }
        return true;
    }
    if (tag == 0 || tag >= UINT64_C(1048576)) {
        scoop_shape_fatal("invalid reference scan size");
    }
    consume(budget, 12 + tag * 8);
    for (uint64_t index = 0; index < tag; index++) {
        uint64_t offset = right[index + 1];
        if (offset % 8 != 0 || (index != 0 && offset <= right[index])) {
            scoop_shape_fatal("reference scan offsets are not canonical");
        }
        if (left[index + 1] != scoop_shape_add(offset, translation)) {
            return false;
        }
    }
    return true;
}

bool scoop_shape_scan_equal(const uint64_t *left, const uint64_t *right,
                            uint64_t translation) {
    ScanComparisonBudget budget = {0};
    return compare_scan(left, right, translation, 1, &budget);
}

static void validate_extent(const uint64_t *scan, uint64_t extent, uint64_t depth,
                            ScanComparisonBudget *budget) {
    if (scan == NULL) {
        return;
    }
    if (depth > 64) {
        scoop_shape_fatal("scan extent validation exceeds its depth limit");
    }
    uint64_t count = scan[0];
    if (count == SCOOP_REFS_ARRAY) {
        consume(budget, 28);
        if (extent < 8 || scan[1] > extent - 8 || scan[2] > extent) {
            scoop_shape_fatal("array scan header exceeds its storage");
        }
        validate_extent((const uint64_t *)(uintptr_t)scan[4], scan[3], depth + 1,
                        budget);
    } else if (count == SCOOP_REFS_SEQUENCE) {
        consume(budget, 12);
        for (uint64_t index = 0; index < scan[1]; index++) {
            validate_extent((const uint64_t *)(uintptr_t)scan[index + 2], extent,
                            depth + 1, budget);
        }
    } else {
        consume(budget, 12 + count * 8);
        for (uint64_t index = 0; index < count; index++) {
            if (extent < 8 || scan[index + 1] > extent - 8) {
                scoop_shape_fatal("reference scan exceeds its storage");
            }
        }
    }
}

void scoop_shape_scan_validate(const uint64_t *scan, uint64_t extent) {
    (void)scoop_shape_scan_equal(scan, scan, 0);
    ScanComparisonBudget budget = {0};
    validate_extent(scan, extent, 1, &budget);
}
