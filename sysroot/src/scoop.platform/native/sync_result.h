#ifndef SCOOP_PLATFORM_SYNC_RESULT_H
#define SCOOP_PLATFORM_SYNC_RESULT_H

#include <stdint.h>

struct ScoopPlatformSyncResult {
    int32_t status;
    int32_t code;
};

static inline struct ScoopPlatformSyncResult sync_status(int32_t error) {
    return (struct ScoopPlatformSyncResult){error == 0 ? 0 : -1, error};
}

#endif
