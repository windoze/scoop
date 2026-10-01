#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/thread.h"
#include "../src/value_shape.h"
#include "platform/image_fixture.h"

static const uint64_t value_scan[] = {1, 0};
static const uint64_t box_scan[] = {1, 16};
static const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 32, 16,
                                      (uint64_t)(uintptr_t)value_scan};
static const ScoopTypeDescriptor leaf_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8},
};
static const ScoopTypeDescriptor zst_box_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_ZERO_SIZED_V1,
                       .minimum_size = 16,
                       .instance_alignment = 16,
                       .inline_offset = 16,
                       .inline_alignment = 16},
};
static const ScoopTypeDescriptor ref_box_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 32,
                       .instance_alignment = 16,
                       .inline_offset = 16,
                       .inline_size = 16,
                       .inline_alignment = 16,
                       .inline_scan = value_scan},
    .object_scan = box_scan,
};
static const ScoopTypeDescriptor zst_array_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_ZERO_SIZED_V1,
                       .minimum_size = 32,
                       .instance_alignment = 16,
                       .inline_offset = 32,
                       .inline_alignment = 16},
};
static const ScoopTypeDescriptor ref_array_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 32,
                       .instance_alignment = 16,
                       .inline_offset = 32,
                       .inline_size = 16,
                       .inline_stride = 16,
                       .inline_alignment = 16,
                       .inline_scan = value_scan},
    .object_scan = array_scan,
};
static const ScoopTypeDescriptor zst_target_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_ZERO_SIZED_V1,
                       .minimum_size = 32,
                       .instance_alignment = 16,
                       .inline_offset = 32,
                       .inline_alignment = 16},
};
static const ScoopTypeDescriptor ref_target_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 32,
                       .instance_alignment = 16,
                       .inline_offset = 32,
                       .inline_size = 16,
                       .inline_stride = 16,
                       .inline_alignment = 16,
                       .inline_scan = value_scan},
    .object_scan = array_scan,
};
static const ScoopTypeDescriptor bytes_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 1,
                       .inline_stride = 1,
                       .inline_alignment = 1},
};

typedef struct TestPayload {
    void *reference;
    uint64_t value;
} TestPayload;

typedef struct TestAnchor {
    _Alignas(16) uintptr_t words[4];
} TestAnchor;
static TestAnchor test_anchor(void) {
    TestAnchor anchor = {{0}};
    anchor.words[2] =
        (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    return anchor;
}

static void *allocate(const ScoopTypeDescriptor *td, size_t size) {
    TestAnchor frame = test_anchor();
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame.words,
                                     (uintptr_t)&frame.words[2]);
    void *object = scoop_gc_alloc_internal(td, size);
    scoop_thread_pop_managed_anchor(&anchor);
    return object;
}

static void *box(const ScoopTypeDescriptor *td, const void *source) {
    TestAnchor frame = test_anchor();
    if (source == NULL) {
        return scoop_rt_box_zst_impl(td, 0x1010, (uintptr_t)frame.words,
                                     (uintptr_t)&frame.words[2]);
    }
    return scoop_rt_box_value_impl(td, source, 0x1010, (uintptr_t)frame.words,
                                   (uintptr_t)&frame.words[2]);
}

static ScoopArray *clone(const void *source, const ScoopTypeDescriptor *source_td,
                         const ScoopTypeDescriptor *target_td) {
    TestAnchor frame = test_anchor();
    return (ScoopArray *)scoop_rt_array_clone_impl(source, source_td, target_td, 0x1010,
                                                   (uintptr_t)frame.words,
                                                   (uintptr_t)&frame.words[2]);
}

static void collect(void) {
    TestAnchor frame = test_anchor();
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame.words,
                             (uintptr_t)&frame.words[2]);
}

typedef void (*AbortProbe)(void *);
static void expect_abort(AbortProbe probe, void *argument) {
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(STDERR_FILENO);
        probe(argument);
        _exit(0);
    }
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
}

static void unrooted_box(void *unused) {
    (void)unused;
    _Alignas(16) TestPayload payload = {NULL, 42};
    (void)box(&ref_box_td, &payload);
}
static void wrong_scan_box(void *unused) {
    (void)unused;
    _Alignas(16) TestPayload payload = {NULL, 42};
    static const uint64_t other_scan[] = {1, 8};
    ScoopNativeRegionRootEntry entry = {&payload, other_scan};
    ScoopNativeRegionRootFrame region;
    scoop_rt_push_native_region_roots(&region, &entry, 1);
    (void)box(&ref_box_td, &payload);
    scoop_rt_pop_native_region_roots(&region);
}
static void interior_unbox(void *object) {
    scoop_rt_unbox_zst((char *)object + 8, &zst_box_td);
}
static void wrong_exact_unbox(void *object) {
    ScoopTypeDescriptor other = zst_box_td;
    scoop_rt_unbox_zst(object, &other);
}
static void wrong_unbox_entry(void *object) {
    _Alignas(16) TestPayload destination;
    scoop_rt_unbox_value(object, &zst_box_td, &destination);
}

static void test_boxing(bool stress) {
    void *first = NULL;
    void *second = NULL;
    void **slots[] = {&first, &second};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 2);
    first = box(&zst_box_td, NULL);
    second = box(&zst_box_td, NULL);
    assert(first != second);
    assert((uintptr_t)first % 16 == 0 && (uintptr_t)second % 16 == 0);
    assert(scoop_rt_gc_debug_allocation_size(first) == 16);
    scoop_rt_unbox_zst(first, &zst_box_td);
    expect_abort(wrong_exact_unbox, first);
    expect_abort(wrong_unbox_entry, first);
    expect_abort(unrooted_box, NULL);
    expect_abort(wrong_scan_box, NULL);
    expect_abort(interior_unbox, first);

    _Alignas(16) TestPayload payload = {allocate(&leaf_td, 24), 42};
    uintptr_t old_reference = (uintptr_t)payload.reference;
    ScoopNativeRegionRootEntry entry = {&payload, value_scan};
    ScoopNativeRegionRootFrame region;
    scoop_rt_push_native_region_roots(&region, &entry, 1);
    first = box(&ref_box_td, &payload);
    if (stress) {
        assert((uintptr_t)payload.reference != old_reference);
    }
    _Alignas(16) TestPayload result = {0};
    scoop_rt_unbox_value(first, &ref_box_td, &result);
    assert(result.reference == payload.reference && result.value == 42);
    collect();
    scoop_rt_unbox_value(first, &ref_box_td, &result);
    assert(result.reference == payload.reference && result.value == 42);
    assert((uintptr_t)first % 16 == 0);
    scoop_rt_pop_native_region_roots(&region);
    scoop_rt_pop_native_roots(&roots);
}

static void invalid_count(void *object) {
    ((ScoopArray *)object)->size = UINT64_MAX;
    (void)clone(object, &zst_array_td, &zst_array_td);
}
static void invalid_side_size(void *object) {
    ((ScoopArray *)object)->size += 1;
    (void)clone(object, &ref_array_td, &ref_array_td);
}
static void wrong_array_exact(void *object) {
    ScoopTypeDescriptor other = zst_array_td;
    (void)clone(object, &other, &zst_array_td);
}

static void test_arrays(void) {
    ScoopArray *source = NULL;
    ScoopArray *copy = NULL;
    void *leaf = NULL;
    void **slots[] = {(void **)&source, (void **)&copy, &leaf};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 3);
    const uint64_t counts[] = {0, 1, INT64_MAX};
    for (size_t i = 0; i < sizeof counts / sizeof counts[0]; i++) {
        source = allocate(&zst_array_td,
                          scoop_shape_allocation_size(&zst_array_td, counts[i]));
        source->size = counts[i];
        copy = clone(source, &zst_array_td, &zst_target_td);
        assert(copy != source && copy->size == counts[i] &&
               copy->header.td == &zst_target_td);
        assert(scoop_rt_gc_debug_allocation_size(copy) == 32);
        collect();
        assert(copy->size == counts[i]);
    }
    expect_abort(invalid_count, source);
    expect_abort(wrong_array_exact, source);
    leaf = allocate(&leaf_td, 24);
    source = allocate(&ref_array_td, scoop_shape_allocation_size(&ref_array_td, 2));
    source->size = 2;
    TestPayload *payload = (TestPayload *)((char *)source + 32);
    payload[0] = (TestPayload){leaf, 71};
    payload[1] = (TestPayload){leaf, 72};
    copy = clone(source, &ref_array_td, &ref_target_td);
    collect();
    payload = (TestPayload *)((char *)copy + 32);
    assert((uintptr_t)copy % 16 == 0 && copy->header.td == &ref_target_td);
    assert(payload[0].reference == leaf && payload[1].reference == leaf);
    assert(payload[0].value == 71 && payload[1].value == 72);
    assert(scoop_rt_gc_debug_allocation_size(copy) == 64);
    expect_abort(invalid_side_size, source);
    scoop_rt_pop_native_roots(&roots);
}

static void validate_bad_shape(void *td) { scoop_shape_validate(td); }
static void overflow_string(void *unused) {
    (void)unused;
    (void)scoop_shape_allocation_size(&bytes_td, INT64_MAX);
}
static void overflow_array(void *unused) {
    (void)unused;
    (void)scoop_shape_allocation_size(&ref_array_td, INT64_MAX);
}
static void abstract_allocation(void *unused) {
    (void)unused;
    ScoopTypeDescriptor td = {
        .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1}};
    (void)allocate(&td, 16);
}
static void test_shape_failures(void) {
    ScoopTypeDescriptor malformed = zst_array_td;
    malformed.instance_shape.inline_scan = value_scan;
    expect_abort(validate_bad_shape, &malformed);
    malformed = ref_array_td;
    uint64_t scan[5];
    memcpy(scan, array_scan, sizeof scan);
    malformed.object_scan = scan;
    for (size_t field = 1; field <= 3; field++) {
        scan[field] += 8;
        expect_abort(validate_bad_shape, &malformed);
        scan[field] -= 8;
    }
    malformed = ref_box_td;
    malformed.object_scan = value_scan;
    expect_abort(validate_bad_shape, &malformed);
    malformed = zst_box_td;
    malformed.instance_shape.inline_alignment = 32;
    expect_abort(validate_bad_shape, &malformed);
    expect_abort(overflow_string, NULL);
    expect_abort(overflow_array, NULL);
    expect_abort(abstract_allocation, NULL);
    assert(scoop_shape_allocation_size(&bytes_td, 0) == 24);
    assert(scoop_shape_allocation_size(&bytes_td, 9) == 40);
}

static void run_mode(bool stress) {
    if (stress) {
        assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
    } else {
        assert(unsetenv("SCOOP_GC_STRESS_MOVE") == 0);
    }
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    static const ScoopTypeDescriptor *const types[] = {
        &leaf_td,      &zst_box_td,    &ref_box_td,    &zst_array_td,
        &ref_array_td, &zst_target_td, &ref_target_td, &bytes_td,
    };
    scoop_test_image_init(types, sizeof types / sizeof *types, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    test_boxing(stress);
    test_arrays();
    test_shape_failures();
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
}

int main(void) {
    for (unsigned mode = 0; mode < 2; mode++) {
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            run_mode(mode != 0);
            _exit(0);
        }
        int status;
        assert(waitpid(child, &status, 0) == child);
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    }
    puts("descriptor-driven representation tests passed");
    return 0;
}
