#ifndef SCOOP_PLATFORM_ENVIRONMENT_H
#define SCOOP_PLATFORM_ENVIRONMENT_H

#include "os_strings.h"

ScoopPlatformString scoop_platform_environment_value_result(const void *name);
ScoopPlatformString scoop_platform_environment_result(void);
ScoopPlatformString scoop_platform_current_directory_result(void);
ScoopPlatformString scoop_platform_home_directory_result(void);

#endif
