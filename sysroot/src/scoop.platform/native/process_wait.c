#define _GNU_SOURCE 1
#include "process_native.h"
#include <stdbool.h>
#ifdef _WIN32

ScoopPlatformProcessWait scoop_platform_process_wait(ScoopPlatformProcess child, bool no_hang) {
    HANDLE process = (HANDLE)(uintptr_t)child.value;
    DWORD waited = WaitForSingleObject(process, no_hang ? 0 : INFINITE);
    if (waited == WAIT_TIMEOUT)
        return (ScoopPlatformProcessWait){-2, 0};
    if (waited == WAIT_FAILED)
        return (ScoopPlatformProcessWait){-1, (int32_t)GetLastError()};
    DWORD status;
    if (!GetExitCodeProcess(process, &status))
        return (ScoopPlatformProcessWait){-1, (int32_t)GetLastError()};
    if (!CloseHandle(process))
        return (ScoopPlatformProcessWait){-3, (int32_t)GetLastError()};
    return (ScoopPlatformProcessWait){status, 0};
}

int32_t scoop_platform_process_signal(ScoopPlatformProcess child, bool force) {
    (void)force;
    HANDLE process = (HANDLE)(uintptr_t)child.value;
    if (TerminateProcess(process, 1))
        return 0;
    DWORD error = GetLastError();
    if (error == ERROR_ACCESS_DENIED && WaitForSingleObject(process, 0) == WAIT_OBJECT_0)
        return 0;
    return (int32_t)error;
}

void scoop_platform_process_abandon(ScoopPlatformProcess child) {
    (void)CloseHandle((HANDLE)(uintptr_t)child.value);
}

#else
#include <errno.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/wait.h>

ScoopPlatformProcessWait scoop_platform_process_wait(ScoopPlatformProcess child, bool no_hang) {
    int status;
    pid_t result;
    do {
        result = waitpid((pid_t)child.value, &status, no_hang ? WNOHANG : 0);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return (ScoopPlatformProcessWait){errno == ECHILD ? -3 : -1, errno};
    if (result == 0)
        return (ScoopPlatformProcessWait){-2, 0};
    return (ScoopPlatformProcessWait){
        WIFEXITED(status) ? WEXITSTATUS(status) : 256 + WTERMSIG(status), 0};
}

int32_t scoop_platform_process_signal(ScoopPlatformProcess child, bool force) {
    int result = kill((pid_t)child.value, force ? SIGKILL : SIGTERM);
    return result < 0 && errno != ESRCH ? errno : 0;
}

void scoop_platform_process_abandon(ScoopPlatformProcess child) {
    (void)waitpid((pid_t)child.value, NULL, WNOHANG);
}
#endif
