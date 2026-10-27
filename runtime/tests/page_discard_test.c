#include <string.h>

#include "nursery_fixture.h"
#include "../src/platform/platform.h"

extern bool scoop_test_vm_fail_discards;
extern void (*scoop_test_vm_discard_observer)(void *base, size_t size);

static uintptr_t live_begin, live_end, region_base, live_block;
static size_t page_size, observed;
static bool discarded_prefix;
static const ScoopTypeDescriptor bytes_td = {
    .type_id = 911,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 512,
                       .instance_alignment = 8}};

static void observe_discard(void *base, size_t size) {
    uintptr_t address = (uintptr_t)base;
    assert(address % page_size == 0 && size != 0 && size % page_size == 0);
    assert(address + size <= live_begin || address >= live_end);
    assert(address != region_base);
    if (live_block != 0 && address <= live_block && address + size >= live_block + page_size) {
        discarded_prefix = true;
    }
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
    ScoopTypeDescriptor whole_block_td = bytes_td;
    whole_block_td.type_id = 913;
    whole_block_td.instance_shape.minimum_size = GC_REGULAR_MAX;
    ScoopTypeDescriptor prefix_filler_td = bytes_td;
    prefix_filler_td.type_id = 914;
    prefix_filler_td.instance_shape.minimum_size = page_size - GC_LINE_SIZE;
    const ScoopTypeDescriptor *types[] = {
        &nursery_node_td, &bytes_td, &filler_td, &whole_block_td, &prefix_filler_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 5, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((const void *)scoop_rt_thread_debug_stack_high());
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
    ScoopGcRegion *region = scoop_heap_region_for_address((uintptr_t)object);
    region_base = region->base;
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
    assert(scoop_heap_known_object_block(object, true)->region == region);
    check_payload(object);
    nursery_collect(false);
    assert(nursery_metrics().discard_calls == succeeded.discard_calls);
    unsigned char *reused = nursery_allocate(&bytes_td, 512);
    assert(scoop_heap_known_object_block(reused, true)->region == region);
    for (size_t offset = sizeof(ScoopObjectHeader); offset < 512; offset++) {
        assert(reused[offset] == 0);
    }
    check_payload(object);
    scoop_rt_pop_pin_frame(&pin);
    live_begin = live_end = 0;
    nursery_collect(false);
    assert(scoop_heap_region_for_address(region_base) == region);
    assert(*(ScoopGcRegion **)region_base == region);

    // A live object in the second page must not keep its empty first page.
    // Only the region's reserved metadata block supplies the owner pointer.
    (void)nursery_allocate(&whole_block_td, GC_REGULAR_MAX);
    (void)nursery_allocate(&prefix_filler_td, page_size - GC_LINE_SIZE);
    object = nursery_allocate(&bytes_td, 512);
    assert((uintptr_t)object % GC_BLOCK_SIZE == page_size);
    live_begin = (uintptr_t)object;
    live_end = live_begin + page_size;
    live_block = live_begin & ~(uintptr_t)(GC_BLOCK_SIZE - 1);
    region = scoop_heap_region_for_address(live_begin);
    region_base = region->base;
    assert(live_block != region_base);
    memset(object + sizeof(ScoopObjectHeader), 93, 512 - sizeof(ScoopObjectHeader));
    scoop_rt_push_pin_frame(&pin, object);
    nursery_collect(false);
    assert(discarded_prefix && *(ScoopGcRegion **)region_base == region);
#if SCOOP_ENABLE_GC_STRESS
    assert(region->blocks[0].state == SCOOP_BLOCK_NEVER_USED);
#endif
    assert(scoop_heap_known_object_block(object, true)->region == region);
    check_payload(object);
    scoop_rt_pop_pin_frame(&pin);
    live_begin = live_end = live_block = 0;
    nursery_collect(false);
    scoop_test_vm_discard_observer = NULL;
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("discard preserves cross-page pins, retries failure and reuses zeroed storage");
}
