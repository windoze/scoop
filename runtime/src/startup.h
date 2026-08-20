#ifndef SCOOP_RT_STARTUP_H
#define SCOOP_RT_STARTUP_H

#include "scoop_runtime_metadata_v1.h"

/* One complete program lifetime. The pointer array and records belong to the
 * loaded main image; no managed code may run before this call registers them. */
int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *images, uint64_t image_count,
                         const ScoopRootEntryDescriptorV1 *root_entry, int32_t argc,
                         const char *const *argv);

#endif
