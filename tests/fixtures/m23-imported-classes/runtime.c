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

#ifndef SCOOP_FIXTURE_METADATA_CHECK
#define SCOOP_FIXTURE_METADATA_CHECK() true
#endif

int main(void) {
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init();
    scoop_thread_attach_main();
    scoop_thread_enter_managed(__builtin_frame_address(0));
    int64_t result = fixture_check();
    bool metadata = SCOOP_FIXTURE_METADATA_CHECK();
    bool collected = scoop_rt_thread_debug_gc_epoch() > 0;
    scoop_callback_prepare_shutdown();
    scoop_eh_prepare_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    bool correct = metadata && result == 42 && (getenv("SCOOP_GC_STRESS_MOVE") == NULL || collected);
    if (!correct) {
        fprintf(stderr, "fixture result=%" PRId64 ", metadata=%d, collected=%d\n",
                result, metadata, collected);
    }
    return correct ? 0 : 1;
}
