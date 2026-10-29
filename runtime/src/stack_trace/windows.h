#ifndef SCOOP_TRACE_WINDOWS_H
#define SCOOP_TRACE_WINDOWS_H

#define WIN32_LEAN_AND_MEAN
#include "internal.h"
#include <windows.h>

char *scoop_trace_utf8(const WCHAR *text);
WCHAR *scoop_trace_module_path(HMODULE module);

#endif
