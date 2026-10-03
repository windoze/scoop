#include <stdio.h>
#include <stdlib.h>

#include "../image/registry.h"
#include "../thread/internal.h"
#include "internal.h"

_Noreturn void scoop_startup_fatal(const char *message) {
    fprintf(stderr, "scoop startup: %s\n", message);
    abort();
}

_Noreturn void
scoop_startup_report_failure(const ScoopStaticStorageDescriptorV1 *failure,
                             const ScoopInitializationUnitDescriptorV1 *unit) {
    /* Only the stable slot crosses the wait. The exception address is read and
     * consumed while the running world is held; the resulting name is readonly. */
    scoop_thread_registry_lock();
    scoop_thread_wait_for_running_world();
    const ScoopObjectHeader *exception =
        *(const ScoopObjectHeader **)failure->writable_base;
    if (exception == NULL || (unit != NULL && unit->cell->state != 3)) {
        scoop_startup_fatal("gateway failure has no published exception");
    }
    scoop_image_require_type(exception->td);
    ScoopByteSpanV1 name = exception->td->diagnostic_name;
    scoop_thread_registry_unlock();
    fputs("scoop: uncaught exception: ", stderr);
    fwrite(name.data, 1, (size_t)name.length, stderr);
    if (unit != NULL) {
        fputs(" during initialization of ", stderr);
        fwrite(unit->diagnostic_path.data, 1, (size_t)unit->diagnostic_path.length,
               stderr);
    }
    fputc('\n', stderr);
    abort();
}
