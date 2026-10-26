#include "no_core.h"
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
                       .instance_alignment = 8}};
static const ScoopTypeDescriptor zst_box_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_ZERO_SIZED_V1,
                       .minimum_size = 16,
                       .instance_alignment = 16,
                       .inline_offset = 16,
                       .inline_alignment = 16}};
static const ScoopTypeDescriptor ref_box_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_BOXED_VALUE_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 32,
                       .instance_alignment = 16,
                       .inline_offset = 16,
                       .inline_size = 16,
                       .inline_alignment = 16,
                       .inline_scan = value_scan},
    .object_scan = box_scan};
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
    .object_scan = array_scan};
static const ScoopTypeDescriptor bytes_td = {
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 1,
                       .inline_stride = 1,
                       .inline_alignment = 1}};

typedef struct TestPayload {
    void *reference;
    uint64_t value;
} TestPayload;

typedef struct TestAnchor {
    _Alignas(16) uintptr_t words[4];
} TestAnchor;
static TestAnchor test_anchor(void) {
    TestAnchor anchor = {{0}};
    anchor.words[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
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

static void collect(void) {
    TestAnchor frame = test_anchor();
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame.words, (uintptr_t)&frame.words[2]);
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

static void interior_unbox(void *object) { scoop_rt_unbox_zst((char *)object + 8, &zst_box_td); }
static void invalid_object_unbox(void *object) { scoop_rt_unbox_zst(object, &zst_box_td); }
static void wrong_exact_unbox(void *object) {
    ScoopTypeDescriptor other = zst_box_td;
    scoop_rt_unbox_zst(object, &other);
}
static void wrong_unbox_entry(void *object) {
    _Alignas(16) TestPayload destination;
    scoop_rt_unbox_value(object, &zst_box_td, &destination);
}

static void test_unboxing(void) {
    void *first = NULL;
    void *second = NULL;
    void **slots[] = {&first, &second};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 2);
    first = allocate(&zst_box_td, 16);
    second = allocate(&zst_box_td, 16);
    assert(first != second);
    assert((uintptr_t)first % 16 == 0 && (uintptr_t)second % 16 == 0);
    assert(scoop_rt_gc_debug_allocation_size(first) == 16);
    scoop_rt_unbox_zst(first, &zst_box_td);
    expect_abort(wrong_exact_unbox, first);
    expect_abort(wrong_unbox_entry, first);
    expect_abort(interior_unbox, first);
    expect_abort(invalid_object_unbox, NULL);
    expect_abort(invalid_object_unbox, (void *)(uintptr_t)8);

    _Alignas(16) TestPayload payload = {allocate(&leaf_td, 24), 42};
    ScoopNativeRegionRootEntry entry = {&payload, value_scan};
    ScoopNativeRegionRootFrame region;
    scoop_rt_push_native_region_roots(&region, &entry, 1);
    first = allocate(&ref_box_td, 32);
    memcpy((char *)first + 16, &payload, sizeof payload);
    scoop_rt_gc_write_barrier((char *)first + 16, sizeof payload);
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

static void *native_allocate(const ScoopTypeDescriptor *td, size_t size) {
    TestAnchor frame = test_anchor();
    void *object = NULL;
    ScoopCallerRootEntry entry = {&object, value_scan};
    ScoopCallerRootFrame roots;
    ScoopThreadTransition transition;
    scoop_rt_push_caller_roots(&roots, &entry, 1);
    scoop_rt_enter_native_borrowed_impl(&transition, (uintptr_t)frame.words, 0x1010,
                                        (uintptr_t)frame.words, (uintptr_t)&frame.words[2]);
    object = scoop_rt_alloc(td, size);
    scoop_rt_leave_native_borrowed(&transition);
    scoop_rt_pop_caller_roots(&roots);
    return object;
}

static void invalid_native_type(void *td) { (void)native_allocate(td, 24); }

static void test_native_allocation_boundary(void) {
    ScoopObjectHeader *object = native_allocate(&leaf_td, 24);
    assert(object->td == &leaf_td);
    assert(scoop_rt_gc_debug_allocation_size(object) == 24);
    assert(*(uint64_t *)(object + 1) == 0);
    ScoopTypeDescriptor unregistered = leaf_td;
    expect_abort(invalid_native_type, &unregistered);
    expect_abort(invalid_native_type, NULL);
    expect_abort(invalid_native_type, (void *)(uintptr_t)1);
}

static void test_allocation_bounds(void) {
    expect_abort(overflow_string, NULL);
    expect_abort(overflow_array, NULL);
    expect_abort(abstract_allocation, NULL);
    assert(scoop_shape_allocation_size(&bytes_td, 0) == 24);
    assert(scoop_shape_allocation_size(&bytes_td, 9) == 40);
}

static void negative_string_byte(void *value) { (void)scoop_rt_string_byte_at_leaf(value, -1); }
static void past_string_byte(void *value) {
    (void)scoop_rt_string_byte_at_leaf(value, (int64_t)((ScoopString *)value)->len);
}
static void negative_string_cursor(void *value) {
    (void)scoop_rt_string_character_at_byte_leaf(value, -1);
}
static void past_string_cursor(void *value) {
    (void)scoop_rt_string_character_at_byte_leaf(value, (int64_t)((ScoopString *)value)->len);
}

static void test_string_reads(void) {
    const unsigned char bytes[] = {'A', 0, 0xc3, 0xa9, 0xe9, 0x9b, 0xaa, 0xf0, 0x9f, 0x99, 0x82};
    ScoopString *value = allocate(&bytes_td, scoop_shape_allocation_size(&bytes_td, sizeof bytes));
    value->len = sizeof bytes;
    memcpy(value->data, bytes, sizeof bytes);
    assert(scoop_rt_string_byte_length_leaf(value) == (int64_t)sizeof bytes);
    for (size_t i = 0; i < sizeof bytes; i++) {
        assert((uint8_t)scoop_rt_string_byte_at_leaf(value, (int64_t)i) == bytes[i]);
    }
    assert(scoop_rt_string_character_at_byte_leaf(value, 0) == 'A');
    assert(scoop_rt_string_character_at_byte_leaf(value, 1) == 0);
    assert(scoop_rt_string_character_at_byte_leaf(value, 2) == 0xe9);
    assert(scoop_rt_string_character_at_byte_leaf(value, 4) == 0x96ea);
    assert(scoop_rt_string_character_at_byte_leaf(value, 7) == 0x1f642);
    expect_abort(negative_string_byte, value);
    expect_abort(past_string_byte, value);
    expect_abort(negative_string_cursor, value);
    expect_abort(past_string_cursor, value);
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
        &leaf_td, &zst_box_td, &ref_box_td, &ref_array_td, &bytes_td,
    };
    scoop_test_image_init(types, sizeof types / sizeof *types, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    test_unboxing();
    test_allocation_bounds();
    test_native_allocation_boundary();
    test_string_reads();
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
}

int main(void) {
    scoop_test_disable_core_dumps();
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
