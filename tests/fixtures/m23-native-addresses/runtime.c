#include "generated_entries.h"
#include "eh_internal.h"
#include "thread.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

/* Static roots and immortal values come from the actual artifact registrations.
 * The fixtures access lazy units through their compiled ensure functions. */
SCOOP_FIXTURE_GLOBALS
SCOOP_FIXTURE_IMMORTALS
const ScoopInitializationUnitDescriptor scoop_image_initialization_units[] = {{0}};
const uint64_t scoop_image_initialization_unit_count = 0;

extern int64_t fixture_check(void) __asm__("SCOOP_FIXTURE_ENTRY");

int64_t m23_call_scalar(int64_t (*callback)(int64_t), int64_t value) {
    return callback(value);
}

struct NativeWide { int64_t first; int64_t second; int64_t third; };
int64_t m23_call_wide(struct NativeWide (*callback)(struct NativeWide)) {
    struct NativeWide value = {17, -29, 54};
    struct NativeWide result = callback(value);
    return result.first + result.second + result.third;
}

int64_t m23_call_sink(void (*callback)(int64_t *)) {
    int64_t value = 0;
    callback(&value);
    return value;
}

int main(void) {
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init();
    scoop_thread_attach_main();
    scoop_thread_enter_managed(__builtin_frame_address(0));
    int64_t result = fixture_check();
    bool collected = scoop_rt_thread_debug_gc_epoch() > 0;
    scoop_callback_prepare_shutdown();
    scoop_eh_prepare_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    bool correct = result == 42 && (getenv("SCOOP_GC_STRESS_MOVE") == NULL || collected);
    if (!correct) {
        fprintf(stderr, "fixture result=%" PRId64 ", collected=%d\n", result, collected);
    }
    return correct ? 0 : 1;
}
