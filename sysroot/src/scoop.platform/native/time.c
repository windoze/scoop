#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#ifdef _WIN32
#include "windows_api.h"
#ifdef _MSC_VER
#include <intrin.h>
#ifndef __clang__
#include <immintrin.h>
#endif
#endif
#else
#include <time.h>
#endif

struct ScoopPlatformTime {
    int64_t seconds, nanoseconds;
    int32_t error;
};

struct ScoopPlatformTime scoop_platform_clock(bool monotonic) {
#ifdef _WIN32
    if (!monotonic) {
        FILETIME value;
        GetSystemTimePreciseAsFileTime(&value);
        uint64_t ticks = (uint64_t)value.dwHighDateTime << 32 | value.dwLowDateTime;
        return (struct ScoopPlatformTime){(int64_t)(ticks / UINT64_C(10000000)) -
                                              INT64_C(11644473600),
                                          (int64_t)(ticks % UINT64_C(10000000)) * 100, 0};
    }
    LARGE_INTEGER counter, frequency;
    if (!QueryPerformanceFrequency(&frequency) || !QueryPerformanceCounter(&counter))
        return (struct ScoopPlatformTime){0, -1, (int32_t)GetLastError()};
    uint64_t ticks = (uint64_t)counter.QuadPart, hz = (uint64_t)frequency.QuadPart;
    uint64_t remainder = ticks % hz;
#ifdef _MSC_VER
    uint64_t high, unused;
    uint64_t low = _umul128(remainder, UINT64_C(1000000000), &high);
#ifdef __clang__
    /* remainder < hz, so the quotient fits in 64 bits for the x64 div instruction. */
    uint64_t nanos;
    __asm__("divq %4" : "=a"(nanos), "=d"(unused) : "a"(low), "d"(high), "r"(hz));
#else
    uint64_t nanos = _udiv128(high, low, hz, &unused);
#endif
#else
    uint64_t nanos = (uint64_t)(((__uint128_t)remainder * UINT64_C(1000000000)) / hz);
#endif
    return (struct ScoopPlatformTime){(int64_t)(ticks / hz), (int64_t)nanos, 0};
#else
    struct timespec value;
    if (clock_gettime(monotonic ? CLOCK_MONOTONIC : CLOCK_REALTIME, &value) < 0)
        return (struct ScoopPlatformTime){0, -1, errno};
    return (struct ScoopPlatformTime){value.tv_sec, value.tv_nsec, 0};
#endif
}

int32_t scoop_platform_sleep(int64_t seconds, int32_t nanoseconds) {
    if (seconds < 0 || nanoseconds < 0 || nanoseconds >= 1000000000) {
#ifdef _WIN32
        return ERROR_INVALID_PARAMETER;
#else
        return EINVAL;
#endif
    }
    if (seconds == 0 && nanoseconds == 0)
        return 0;
    struct ScoopPlatformTime now = scoop_platform_clock(true);
    if (now.error != 0)
        return now.error;
    int64_t nanos = now.nanoseconds + (int64_t)nanoseconds;
    int64_t carry = nanos / 1000000000;
    if (seconds > INT64_MAX - now.seconds - carry) {
#ifdef _WIN32
        return ERROR_ARITHMETIC_OVERFLOW;
#else
        return EOVERFLOW;
#endif
    }
    int64_t deadline_seconds = now.seconds + seconds + carry;
    int64_t deadline_nanos = nanos % 1000000000;
    for (;;) {
        if (now.seconds > deadline_seconds ||
            (now.seconds == deadline_seconds && now.nanoseconds >= deadline_nanos))
            return 0;
        int64_t left_seconds = deadline_seconds - now.seconds;
        int64_t left_nanos = deadline_nanos - now.nanoseconds;
        if (left_nanos < 0) {
            --left_seconds;
            left_nanos += 1000000000;
        }
#ifdef _WIN32
        const uint64_t maximum = INFINITE - 1;
        uint64_t milliseconds = maximum;
        if ((uint64_t)left_seconds <= maximum / 1000) {
            milliseconds =
                (uint64_t)left_seconds * 1000 + ((uint64_t)left_nanos + 999999) / 1000000;
            if (milliseconds > maximum)
                milliseconds = maximum;
        }
        Sleep((DWORD)milliseconds);
#else
        struct timespec remaining = {(time_t)left_seconds, (long)left_nanos};
        if (nanosleep(&remaining, NULL) == 0)
            return 0;
        if (errno != EINTR)
            return errno;
#endif
        now = scoop_platform_clock(true);
        if (now.error != 0)
            return now.error;
    }
}
