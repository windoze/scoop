#include "mbedtls/sha256.h"

#include "../platform.h"

bool scoop_platform_sha256(const void *bytes, size_t length,
                           uint8_t digest[32]) {
    if (digest == NULL || (bytes == NULL && length != 0)) {
        return false;
    }
    return mbedtls_sha256(bytes, length, digest, 0) == 0;
}
