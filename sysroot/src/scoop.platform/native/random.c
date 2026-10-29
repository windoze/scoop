#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "random.h"
#include <errno.h>
#include <stdint.h>
#ifdef _WIN32
#define WIN32_NO_STATUS
#include "windows_api.h"
#undef WIN32_NO_STATUS
#include <bcrypt.h>
#include <ntstatus.h>
#else
#include <sys/random.h>
#endif

struct ScoopPlatformEntropy scoop_platform_entropy(void *data, int64_t count) {
    if (count < 0) {
#ifdef _WIN32
        return (struct ScoopPlatformEntropy){-1, (int32_t)STATUS_INVALID_PARAMETER};
#else
        return (struct ScoopPlatformEntropy){-1, EINVAL};
#endif
    }
    if (count == 0)
        return (struct ScoopPlatformEntropy){0, 0};
#ifdef _WIN32
    ULONG size = (uint64_t)count > UINT32_MAX ? UINT32_MAX : (ULONG)count;
    NTSTATUS status = BCryptGenRandom(NULL, data, size, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
    return BCRYPT_SUCCESS(status) ? (struct ScoopPlatformEntropy){size, 0}
                                  : (struct ScoopPlatformEntropy){-1, (int32_t)status};
#else
    size_t size = count > 256 ? 256 : (size_t)count;
    int64_t result;
    do {
#ifdef __APPLE__
        result = getentropy(data, size) == 0 ? (int64_t)size : -1;
#else
        result = getrandom(data, size, 0);
#endif
    } while (result < 0 && errno == EINTR);
    if (result <= 0)
        return (struct ScoopPlatformEntropy){-1, result == 0 ? EIO : errno};
    return (struct ScoopPlatformEntropy){result, 0};
#endif
}
