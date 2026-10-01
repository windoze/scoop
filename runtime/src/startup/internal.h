#ifndef SCOOP_RT_STARTUP_INTERNAL_H
#define SCOOP_RT_STARTUP_INTERNAL_H

#include "../startup.h"

_Noreturn void scoop_startup_fatal(const char *message);
_Noreturn void
scoop_startup_report_failure(const ScoopStaticStorageDescriptorV1 *failure,
                             const ScoopInitializationUnitDescriptorV1 *unit);

#endif
