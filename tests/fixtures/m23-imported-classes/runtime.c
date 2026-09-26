#include "generated_entries.h"
#include "eh_internal.h"
#include "thread.h"
#include <stdlib.h>

/* The linked fixture has no global storage, initialization units, or immortal
 * values. Supply the empty tables required by the existing single-image entry.
 * All object descriptors and managed bodies come from the actual artifacts. */
const ScoopManagedGlobalDescriptor scoop_image_managed_globals[] = {{0}};
const uint64_t scoop_image_managed_global_count = 0;
const ScoopImmortalObjectDescriptor scoop_image_immortal_objects[] = {{0}};
const uint64_t scoop_image_immortal_object_count = 0;
const ScoopInitializationUnitDescriptor scoop_image_initialization_units[] = {{0}};
const uint64_t scoop_image_initialization_unit_count = 0;

extern int64_t fixture_check(void) __asm__("SCOOP_FIXTURE_ENTRY");

int main(void) {
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init();
    scoop_thread_attach_main(__builtin_frame_address(0));
    int64_t result = fixture_check();
    bool collected = scoop_rt_thread_debug_gc_epoch() > 0;
    scoop_callback_prepare_shutdown();
    scoop_eh_prepare_shutdown();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    return result == 42 && (getenv("SCOOP_GC_STRESS_MOVE") == NULL || collected) ? 0 : 1;
}
