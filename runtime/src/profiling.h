#ifndef SCOOP_PROFILING_H
#define SCOOP_PROFILING_H

#include <stdint.h>
#include <stdio.h>

#if defined(SCOOP_ENABLE_PROFILING) && SCOOP_ENABLE_PROFILING
void scoop_profile_initialize(void);
void scoop_profile_runtime_ready(void);
void scoop_profile_finish(const char *reason, int32_t exit_code);
FILE *scoop_profile_metrics_file(void);
#else
#define scoop_profile_initialize() ((void)0)
#define scoop_profile_runtime_ready() ((void)0)
#define scoop_profile_finish(reason, exit_code) ((void)0)
#define scoop_profile_metrics_file() ((FILE *)0)
#endif

#endif
