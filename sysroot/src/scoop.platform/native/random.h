#ifndef SCOOP_PLATFORM_RANDOM_H
#define SCOOP_PLATFORM_RANDOM_H

#include <stdint.h>

struct ScoopPlatformEntropy {
    int64_t count;
    int32_t error;
};

struct ScoopPlatformEntropy scoop_platform_entropy(void *data, int64_t count);

#endif
