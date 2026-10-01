#include <CommonCrypto/CommonDigest.h>
#include <stdint.h>

#include "../platform.h"

bool scoop_platform_sha256(const void *bytes, size_t length, uint8_t digest[32]) {
    if (length > UINT32_MAX) {
        return false;
    }
    return CC_SHA256(bytes, (CC_LONG)length, digest) != NULL;
}
