#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include <stdint.h>
#ifdef _WIN32
#include "windows_api.h"
#include <winternl.h>

NTSYSAPI NTSTATUS NTAPI RtlGetVersion(PRTL_OSVERSIONINFOW version);
#else
#include <errno.h>
#include <signal.h>
#include <string.h>
#include <sys/utsname.h>
#include <unistd.h>
#endif

struct ScoopPlatformProcessorCount {
    int64_t count;
    int32_t error;
};

struct ScoopPlatformProcessorCount scoop_platform_processor_count(void) {
#ifdef _WIN32
    DWORD count = GetActiveProcessorCount(ALL_PROCESSOR_GROUPS);
    if (count == 0)
        return (struct ScoopPlatformProcessorCount){-1, (int32_t)GetLastError()};
    return (struct ScoopPlatformProcessorCount){count, 0};
#else
    errno = 0;
    long count = sysconf(_SC_NPROCESSORS_ONLN);
    if (count <= 0)
        return (struct ScoopPlatformProcessorCount){-1, errno == 0 ? EIO : errno};
    return (struct ScoopPlatformProcessorCount){count, 0};
#endif
}

#ifdef _WIN32
struct ScoopPlatformSystemVersion {
    uint32_t major, minor, build;
    int32_t error;
};

struct ScoopPlatformSystemVersion scoop_platform_system_version(void) {
    RTL_OSVERSIONINFOW version = {0};
    version.dwOSVersionInfoSize = sizeof(version);
    NTSTATUS status = RtlGetVersion(&version);
    if (status < 0)
        return (struct ScoopPlatformSystemVersion){0, 0, 0, (int32_t)status};
    return (struct ScoopPlatformSystemVersion){version.dwMajorVersion, version.dwMinorVersion,
                                               version.dwBuildNumber, 0};
}
#else
int scoop_platform_process_suspend(void) { return kill(getpid(), SIGTSTP); }

char *scoop_platform_system_version(void) {
    struct utsname value;
    if (uname(&value) < 0)
        return NULL;
    return strdup(value.release);
}
#endif
