#include <stdio.h>
#include <stdlib.h>

#include "../image/registry.h"
#include "../thread/internal.h"
#include "internal.h"
#include "../throwable.h"

_Noreturn void scoop_startup_fatal(const char *message) {
    fprintf(stderr, "scoop startup: %s\n", message);
    abort();
}

_Noreturn void scoop_startup_report_failure(const ScoopStaticStorageDescriptorV1 *failure,
                                            const ScoopInitializationUnitDescriptorV1 *unit) {
    ScoopThrowableReport report = {0};
    for (;;) {
        scoop_thread_registry_lock();
        scoop_thread_wait_for_running_world();
        const ScoopThrowable *exception = *(const ScoopThrowable **)failure->writable_base;
        if (exception == NULL ||
            (unit != NULL && __atomic_load_n(&unit->cell->state, __ATOMIC_RELAXED) != 3)) {
            scoop_startup_fatal("gateway failure has no published exception");
        }
        scoop_image_require_type(exception->header.td);
        size_t required = scoop_throwable_report_copy(exception, &report);
        scoop_thread_registry_unlock();
        if (required == 0) break;
        scoop_throwable_report_reserve(&report, required);
    }
    fputs("scoop: uncaught exception: ", stderr);
    fwrite(report.bytes, 1, report.name_length, stderr);
    if (unit != NULL) {
        fputs(" during initialization of ", stderr);
        fwrite(unit->diagnostic_path.data, 1, (size_t)unit->diagnostic_path.length, stderr);
    }
    fputc('\n', stderr);
    scoop_throwable_report_render_frames(&report, stderr);
    free(report.bytes);
    scoop_rt_exit(1);
}
