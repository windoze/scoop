#include <assert.h>
#include <stddef.h>
#include <stdio.h>

#include "../src/gc_binding.h"
#include "scoop_rt.h"

typedef struct Value {
    void *first;
    uintptr_t metadata;
    void *second;
    uintptr_t padding;
} Value;

typedef struct Array {
    void *header_reference;
    uint64_t count;
    Value elements[6];
} Array;

typedef struct Visited {
    void **slots[32];
    size_t count;
} Visited;

static const uint64_t pair_scan[] = {2, offsetof(Value, first), offsetof(Value, second)};
static const uint64_t second_scan[] = {1, offsetof(Value, second)};
static const uint64_t header_scan[] = {1, offsetof(Array, header_reference)};

static void record(void **slot, void *context) {
    Visited *visited = context;
    assert(visited->count < sizeof visited->slots / sizeof visited->slots[0]);
    visited->slots[visited->count++] = slot;
}

static void check(void *base, const uint64_t *scan, const Visited *expected, uintptr_t begin,
                  uintptr_t end) {
    Visited actual = {0};
    scoop_gc_scan_descriptor(base, scan, record, &actual, begin, end);
    size_t next = 0;
    for (size_t index = 0; index < expected->count; index++) {
        void **slot = expected->slots[index];
        if ((uintptr_t)slot >= begin && (uintptr_t)slot < end) {
            assert(next < actual.count && actual.slots[next++] == slot);
        }
    }
    assert(next == actual.count);
}

static void check_ranges(void *base, size_t size, const uint64_t *scan, const Visited *expected) {
    check(base, scan, expected, 0, UINTPTR_MAX);
    uintptr_t start = (uintptr_t)base;
    /* Include unaligned cuts inside pointer words and entirely empty ranges. */
    for (size_t begin = 0; begin <= size + 8; begin++) {
        for (size_t end = begin; end <= size + 8; end++) {
            check(base, scan, expected, start + begin, start + end);
        }
        check(base, scan, expected, start + begin, UINTPTR_MAX);
    }
}

static void replace(void **slot, void *context) { *slot = context; }

static void flat_and_sequence(void) {
    Array array = {.count = 6};
    for (size_t index = 0; index < 6; index++) {
        array.elements[index].metadata = 0xabad;
        array.elements[index].padding = 0xcafe;
    }
    for (size_t references = 1; references <= 2; references++) {
        const uint64_t *element_scan = references == 1 ? second_scan : pair_scan;
        uint64_t scan[] = {SCOOP_REFS_ARRAY, offsetof(Array, count), offsetof(Array, elements),
                           sizeof(Value), (uintptr_t)element_scan};
        Visited expected = {0};
        for (size_t index = 0; index < 6; index++) {
            if (references == 2) {
                record(&array.elements[index].first, &expected);
            }
            record(&array.elements[index].second, &expected);
        }
        check_ranges(&array, sizeof array, scan, &expected);
        scoop_gc_scan_descriptor(&array, scan, replace, &array, 0, UINTPTR_MAX);
        for (size_t index = 0; index < 6; index++) {
            assert(array.elements[index].first == (references == 2 ? &array : NULL));
            assert(array.elements[index].second == &array);
            assert(array.elements[index].metadata == 0xabad);
            assert(array.elements[index].padding == 0xcafe);
        }

        uint64_t sequence[] = {SCOOP_REFS_SEQUENCE, 2, (uintptr_t)header_scan, (uintptr_t)scan};
        Visited combined = {0};
        record(&array.header_reference, &combined);
        for (size_t index = 0; index < expected.count; index++) {
            record(expected.slots[index], &combined);
        }
        check_ranges(&array, sizeof array, sequence, &combined);
        array.count = 0;
        check_ranges(&array, sizeof array, scan, &(Visited){0});
        array.count = 6;
    }
    check_ranges(&array, sizeof array, NULL, &(Visited){0});
    check_ranges(&array, sizeof array, (uint64_t[]){0}, &(Visited){0});
}

static void recursive_array(void) {
    struct {
        uint64_t count;
        Array elements[2];
    } outer = {.count = 2, .elements = {{.count = 2}, {.count = 3}}};
    uint64_t inner[] = {SCOOP_REFS_ARRAY, offsetof(Array, count), offsetof(Array, elements),
                        sizeof(Value), (uintptr_t)pair_scan};
    uint64_t child[] = {SCOOP_REFS_SEQUENCE, 2, (uintptr_t)header_scan, (uintptr_t)inner};
    uint64_t scan[] = {SCOOP_REFS_ARRAY, 0, sizeof(uint64_t), sizeof(Array), (uintptr_t)child};
    Visited expected = {0};
    for (size_t index = 0; index < 2; index++) {
        Array *array = &outer.elements[index];
        record(&array->header_reference, &expected);
        for (size_t element = 0; element < array->count; element++) {
            record(&array->elements[element].first, &expected);
            record(&array->elements[element].second, &expected);
        }
    }
    check_ranges(&outer, sizeof outer, scan, &expected);
}

int main(void) {
    flat_and_sequence();
    recursive_array();
    puts("flat and recursive array scans preserve exact slot ranges, order and metadata");
    return 0;
}
