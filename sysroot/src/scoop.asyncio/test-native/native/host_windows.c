#include "../../../scoop.platform/native/windows_api.h"
#include <stdbool.h>
#include <stdint.h>

int scoop_test_host_wait(void *event, int timeout) {
    DWORD result = WaitForSingleObject(event, timeout < 0 ? INFINITE : (DWORD)timeout);
    return result == WAIT_OBJECT_0 ? 1 : result == WAIT_TIMEOUT ? 0 : -1;
}

bool scoop_test_event_open(void *event) {
    DWORD flags;
    return GetHandleInformation(event, &flags) != 0;
}

int64_t scoop_test_host_clock(int kind) {
    if (kind) {
        FILETIME created, exited, kernel, user;
        if (!GetProcessTimes(GetCurrentProcess(), &created, &exited, &kernel, &user))
            return -1;
        uint64_t ticks = ((uint64_t)kernel.dwHighDateTime << 32) | kernel.dwLowDateTime;
        ticks += ((uint64_t)user.dwHighDateTime << 32) | user.dwLowDateTime;
        return (int64_t)ticks * 100;
    }
    LARGE_INTEGER counter, frequency;
    if (!QueryPerformanceCounter(&counter) || !QueryPerformanceFrequency(&frequency))
        return -1;
    return (int64_t)((long double)counter.QuadPart * 1000000000 / frequency.QuadPart);
}
