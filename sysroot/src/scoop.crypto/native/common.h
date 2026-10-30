#ifndef SCOOP_CRYPTO_COMMON_H
#define SCOOP_CRYPTO_COMMON_H

#include <openssl/opensslconf.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
  uint64_t value, error;
} ScoopCryptoResult;
typedef struct {
  size_t size;
  unsigned char data[];
} ScoopCryptoBytes;

ScoopCryptoResult scoop_crypto_ok(uint64_t value);
ScoopCryptoResult scoop_crypto_fail(const char *fallback);
int scoop_crypto_begin(void);
ScoopCryptoBytes *scoop_crypto_bytes_new(size_t size);
void scoop_crypto_bytes_free(uint64_t bytes);
ScoopCryptoResult scoop_crypto_bytes_copy(const void *data, size_t size);
const char *scoop_crypto_digest_name(int algorithm);

#endif
