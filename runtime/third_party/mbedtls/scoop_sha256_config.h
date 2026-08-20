#ifndef SCOOP_MBEDTLS_SHA256_CONFIG_H
#define SCOOP_MBEDTLS_SHA256_CONFIG_H

#define MBEDTLS_SHA256_C
#define MBEDTLS_SHA256_SMALLER

/* Keep the embedded module private to Scoop, including when a native library
 * uses another Mbed TLS build in the same executable. */
#define mbedtls_sha256_init scoop_mbedtls_sha256_init
#define mbedtls_sha256_free scoop_mbedtls_sha256_free
#define mbedtls_sha256_clone scoop_mbedtls_sha256_clone
#define mbedtls_sha256_starts scoop_mbedtls_sha256_starts
#define mbedtls_sha256_update scoop_mbedtls_sha256_update
#define mbedtls_sha256_finish scoop_mbedtls_sha256_finish
#define mbedtls_sha256 scoop_mbedtls_sha256
#define mbedtls_internal_sha256_process scoop_mbedtls_internal_sha256_process
#define mbedtls_platform_zeroize scoop_mbedtls_platform_zeroize
#define mbedtls_zeroize_and_free scoop_mbedtls_zeroize_and_free

#endif
