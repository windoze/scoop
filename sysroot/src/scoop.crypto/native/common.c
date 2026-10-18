#include "common.h"
#include <stdio.h>
#include <string.h>

typedef struct {
  unsigned long code;
  char text[256];
} CryptoError;

static const CryptoError no_memory = {0, "native allocation failed"};

ScoopCryptoResult scoop_crypto_ok(uint64_t value) {
  return (ScoopCryptoResult){value, 0};
}

ScoopCryptoResult scoop_crypto_fail(const char *fallback) {
  unsigned long code = ERR_get_error();
  CryptoError *error = OPENSSL_malloc(sizeof(*error));
  if (error) {
    error->code = code;
    if (code)
      ERR_error_string_n(code, error->text, sizeof(error->text));
    else
      snprintf(error->text, sizeof(error->text), "%s", fallback);
  }
  while (ERR_get_error()) {
  }
  return (ScoopCryptoResult){0, (uintptr_t)(error ? error : &no_memory)};
}

int scoop_crypto_begin(void) {
  ERR_clear_error();
  return OPENSSL_init_crypto(OPENSSL_INIT_NO_LOAD_CONFIG, NULL);
}

uint64_t scoop_crypto_error_code(uint64_t value) {
  return ((const CryptoError *)(uintptr_t)value)->code;
}

const char *scoop_crypto_error_text(uint64_t value) {
  return ((const CryptoError *)(uintptr_t)value)->text;
}

void scoop_crypto_error_free(uint64_t value) {
  CryptoError *error = (CryptoError *)(uintptr_t)value;
  if (error != &no_memory)
    OPENSSL_free(error);
}

ScoopCryptoBytes *scoop_crypto_bytes_new(size_t size) {
  if (size > INT64_MAX || size > SIZE_MAX - sizeof(ScoopCryptoBytes))
    return NULL;
  ScoopCryptoBytes *bytes = OPENSSL_malloc(sizeof(*bytes) + size);
  if (bytes)
    bytes->size = size;
  return bytes;
}

ScoopCryptoResult scoop_crypto_bytes_copy(const void *data, size_t size) {
  ScoopCryptoBytes *bytes = scoop_crypto_bytes_new(size);
  if (!bytes)
    return scoop_crypto_fail("native allocation failed");
  if (size)
    memcpy(bytes->data, data, size);
  return scoop_crypto_ok((uintptr_t)bytes);
}

int64_t scoop_crypto_bytes_size(uint64_t value) {
  return (int64_t)((const ScoopCryptoBytes *)(uintptr_t)value)->size;
}

const unsigned char *scoop_crypto_bytes_data(uint64_t value) {
  return ((const ScoopCryptoBytes *)(uintptr_t)value)->data;
}

void scoop_crypto_bytes_free(uint64_t value) {
  ScoopCryptoBytes *bytes = (ScoopCryptoBytes *)(uintptr_t)value;
  if (bytes)
    OPENSSL_clear_free(bytes, sizeof(*bytes) + bytes->size);
}

ScoopCryptoResult scoop_crypto_secret_import(const unsigned char *data,
                                             int64_t size) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  if (size < 0)
    return scoop_crypto_fail("negative byte count");
  return scoop_crypto_bytes_copy(data, (size_t)size);
}

ScoopCryptoResult scoop_crypto_secret_export(uint64_t value) {
  const ScoopCryptoBytes *bytes = (const ScoopCryptoBytes *)(uintptr_t)value;
  ERR_clear_error();
  return scoop_crypto_bytes_copy(bytes->data, bytes->size);
}
