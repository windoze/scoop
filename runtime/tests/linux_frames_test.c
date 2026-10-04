#include <assert.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>

#include "../src/platform/platform.h"
#include "../src/string_abi.h"

extern const ScoopManagedFrameOps scoop_amd64_managed_frame_ops;
extern const ScoopThreadVmOps scoop_linux_thread_vm_ops;
extern const uint8_t test_stackmaps_start[], test_stackmaps_end[];
extern char __executable_start[], etext[];
extern void *probe0(void *), *probe1(void *), *probe2(void *), *probe3(void *);
extern void *outer(void *);
extern uint64_t string_results(const ScoopString *);

static ScoopStackMapIndex stackmaps;
static uintptr_t boundary;
static int old_value, new_value;
static unsigned expected_depth, visits;

static void relocate(uintptr_t pc, uintptr_t sp, uintptr_t fp) {
    const ScoopManagedFrameOps *ops = &scoop_amd64_managed_frame_ops;
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    assert(scoop_linux_thread_vm_ops.stack_bounds(&bounds, &error));
    unsigned depth = 0;
    for (;;) {
        const ScoopStackMapRecord *record =
            scoop_stackmap_lookup(&stackmaps, pc);
        assert(record != NULL && record->root_count == 1);
        ScoopManagedAnchor anchor = {pc, sp, fp, NULL};
        ScoopManagedFrame frame;
        assert(ops->frame_from_anchor(&anchor, record, bounds, &frame, &error));
        void **slot;
        assert(ops->resolve_root(&frame, 0, &slot, &error));
        assert(*slot == &old_value);
        *slot = &new_value;
        ++depth;
        bool has_next;
        assert(ops->next_frame(&frame, boundary, bounds, &pc, &sp, &fp,
                               &has_next, &error));
        if (!has_next) {
            break;
        }
    }
    assert(depth == expected_depth);
    ++visits;
}

void scoop_rt_safepoint_impl(uintptr_t pc, uintptr_t sp, uintptr_t fp) {
    relocate(pc, sp, fp);
}
void scoop_rt_box_zst_impl(uint64_t a, uintptr_t pc, uintptr_t sp,
                           uintptr_t fp) {
    assert(a == 11);
    relocate(pc, sp, fp);
}
void scoop_rt_box_value_impl(uint64_t a, uint64_t b, uintptr_t pc, uintptr_t sp,
                             uintptr_t fp) {
    assert(a == 11 && b == 22);
    relocate(pc, sp, fp);
}
void scoop_rt_array_clone_impl(uint64_t a, uint64_t b, uint64_t c, uintptr_t pc,
                               uintptr_t sp, uintptr_t fp) {
    assert(a == 11 && b == 22 && c == 33);
    relocate(pc, sp, fp);
}

static void reject_bad_roots(void) {
    const ScoopManagedFrameOps *ops = &scoop_amd64_managed_frame_ops;
    ScoopStackMapLocation location = {SCOOP_STACKMAP_INDIRECT, 8, 7, 8, 0};
    ScoopStackMapRootPair pair = {location, location};
    ScoopStackMapRecord record = {
        .stack_size = 24, .root_count = 1, .roots = &pair};
    ScoopPlatformError error = {0};
    assert(ops->validate_record(&record, &error));
    for (int32_t offset = 16; offset <= 24; offset += 8) {
        pair.base.offset = pair.derived.offset = offset;
        assert(!ops->validate_record(&record, &error));
        assert(error.code == SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME);
    }
    pair.base = pair.derived =
        (ScoopStackMapLocation){SCOOP_STACKMAP_INDIRECT, 8, 6, -8, 0};
    assert(ops->validate_record(&record, &error));
    pair.derived.offset = -16;
    assert(!ops->validate_record(&record, &error));
    pair.base = pair.derived =
        (ScoopStackMapLocation){SCOOP_STACKMAP_REGISTER, 8, 0, 0, 0};
    assert(!ops->validate_record(&record, &error));
    record.stack_size = 16;
    assert(!ops->validate_record(&record, &error));
}

static void *thread_bounds(void *unused) {
    (void)unused;
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    assert(scoop_linux_thread_vm_ops.stack_bounds(&bounds, &error));
    assert((uintptr_t)&bounds >= (uintptr_t)bounds.low);
    assert((uintptr_t)&bounds + sizeof bounds <= (uintptr_t)bounds.high);
    return NULL;
}

int main(void) {
    ScoopStackMapImage image = {
        .section = test_stackmaps_start,
        .section_size = (size_t)(test_stackmaps_end - test_stackmaps_start),
        .text_start = (uintptr_t)__executable_start,
        .text_end = (uintptr_t)etext,
    };
    ScoopStackMapError parse_error;
    if (!scoop_stackmap_build_index(&image, 1, &stackmaps, &parse_error)) {
        fprintf(stderr, "stackmap error at %zu: %s\n",
                parse_error.section_offset,
                scoop_stackmap_error_message(parse_error.code));
        abort();
    }
    assert(stackmaps.record_count == 5);
    ScoopPlatformError error = {0};
    for (size_t i = 0; i < stackmaps.record_count; ++i) {
        assert(scoop_amd64_managed_frame_ops.validate_record(
            &stackmaps.records[i], &error));
    }
    boundary = (uintptr_t)__builtin_frame_address(0);
    expected_depth = 1;
    assert(probe0(&old_value) == &new_value);
    assert(probe1(&old_value) == &new_value);
    assert(probe2(&old_value) == &new_value);
    assert(probe3(&old_value) == &new_value);
    expected_depth = 2;
    assert(outer(&old_value) == &new_value);
    assert(visits == 5);
    reject_bad_roots();
    const struct {
        ScoopString header;
        char bytes[5];
    } string = {{.len = 5}, {'a', '\xe4', '\xb8', '\xad', 'b'}};
    assert(string_results(&string.header) == 1);
    thread_bounds(NULL);
    pthread_t thread;
    assert(pthread_create(&thread, NULL, thread_bounds, NULL) == 0);
    assert(pthread_join(thread, NULL) == 0);
    size_t page = scoop_linux_thread_vm_ops.page_size();
    void *mapping;
    assert(page != 0);
    assert(scoop_linux_thread_vm_ops.reserve_read_write(0, page, &mapping,
                                                        &error));
    *(volatile char *)mapping = 42;
    assert(!scoop_linux_thread_vm_ops.protect_none((char *)mapping + 1, page,
                                                   &error));
    assert(scoop_linux_thread_vm_ops.protect_none(mapping, page, &error));
    assert(munmap(mapping, page) == 0);
    scoop_stackmap_dispose_index(&stackmaps);
    puts("amd64 anchors, relocated roots, String sret and Linux VM passed");
}
