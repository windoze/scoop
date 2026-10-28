#ifndef SCOOP_PLATFORM_PROCESS_WINDOWS_H
#define SCOOP_PLATFORM_PROCESS_WINDOWS_H

#include "process_native.h"

WCHAR *scoop_process_command_line(const WCHAR *arguments, int64_t count, DWORD *error);
WCHAR *scoop_process_environment(const WCHAR *environment, int64_t count, DWORD *error);
WCHAR *scoop_process_absolute_path(const WCHAR *path, DWORD *error);

#endif
