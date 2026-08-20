#include <string.h>

#include "nursery_fixture.h"
#include "../src/platform/platform.h"

extern bool scoop_test_vm_fail_discards;
extern void (*scoop_test_vm_discard_observer)(void *base, size_t size);

static uintptr_t live_begin, live_end;
static size_t page_size, observed;
static const ScoopTypeDescriptor bytes_td = {
    .type_id = 911,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 512,
                       .instance_alignment = 8},
};

static void observe_discard(void *base, size_t size) {
    uintptr_t address = (uintptr_t)base;
    assert(address % page_size == 0 && size != 0 && size % page_size == 0);
    assert(address + size <= live_begin || address >= live_end);
    observed++;
}

static void check_payload(const unsigned char *object) {
    for (size_t offset = sizeof(ScoopObjectHeader); offset < 512; offset++) {
        assert(object[offset] == 93);
    }
}

int main(void) {
    page_size = scoop_platform_bundle()->thread_vm->page_size();
    ScoopTypeDescriptor filler_td = bytes_td;
    filler_td.type_id = 912;
    filler_td.instance_shape.minimum_size = page_size - 384;
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &bytes_td, &filler_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    assert(nursery_node(1)->value == 1);
    nursery_collect(false);
    (void)nursery_allocate(&filler_td, page_size - 384);
    unsigned char *object = nursery_allocate(&bytes_td, 512);
    assert((uintptr_t)object % page_size == page_size - 256);
    memset(object + sizeof(ScoopObjectHeader), 93, 512 - sizeof(ScoopObjectHeader));
    ScoopPinFrame pin;
    scoop_rt_push_pin_frame(&pin, object);
    live_begin = (uintptr_t)object / page_size * page_size;
    live_end = live_begin + 2 * page_size;
    for (size_t index = 0; index < 1000; index++) {
        (void)nursery_allocate(&bytes_td, 512);
    }
    ScoopGcMetrics before = nursery_metrics();
    scoop_test_vm_discard_observer = observe_discard;
    scoop_test_vm_fail_discards = true;
    nursery_collect(false);
    ScoopGcMetrics failed = nursery_metrics();
    assert(observed != 0 && failed.discard_failures > before.discard_failures);
    assert(failed.discarded_bytes == before.discarded_bytes);
    check_payload(object);

    scoop_test_vm_fail_discards = false;
    nursery_collect(false);
    ScoopGcMetrics succeeded = nursery_metrics();
    assert(succeeded.discard_calls > before.discard_calls);
    assert(succeeded.discarded_bytes > before.discarded_bytes);
    assert(succeeded.region_count == 1 && succeeded.unmapped_bytes == before.unmapped_bytes);
    assert(succeeded.current_rss_bytes > 0 && succeeded.peak_rss_bytes > 0);
    check_payload(object);
    nursery_collect(false);
    assert(nursery_metrics().discard_calls == succeeded.discard_calls);
    unsigned char *reused = nursery_allocate(&bytes_td, 512);
    for (size_t offset = sizeof(ScoopObjectHeader); offset < 512; offset++) {
        assert(reused[offset] == 0);
    }
    check_payload(object);
    scoop_test_vm_discard_observer = NULL;
    scoop_rt_pop_pin_frame(&pin);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("discard preserves cross-page pins, retries failure and reuses zeroed storage");
}
