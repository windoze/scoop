#include "common.h"
#include <openssl/rand.h>

const char *scoop_crypto_digest_name(int algorithm) {
  switch (algorithm) {
  case 0:
    return "SHA256";
  case 1:
    return "SHA384";
  case 2:
    return "SHA512";
  case 3:
    return "SHA1";
  case 4:
    return "MD5";
  case 5:
    return "SHA512-256";
  default:
    return NULL;
  }
}

ScoopCryptoResult scoop_crypto_digest_new(int algorithm) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  const char *name = scoop_crypto_digest_name(algorithm);
  EVP_MD_CTX *ctx = EVP_MD_CTX_new();
  EVP_MD *md = name ? EVP_MD_fetch(NULL, name, NULL) : NULL;
  int ok = ctx && md && EVP_DigestInit_ex(ctx, md, NULL);
  EVP_MD_free(md);
  if (!ok) {
    ScoopCryptoResult error = scoop_crypto_fail("invalid digest algorithm");
    EVP_MD_CTX_free(ctx);
    return error;
  }
  return scoop_crypto_ok((uintptr_t)ctx);
}

ScoopCryptoResult scoop_crypto_digest_update(uint64_t value, const void *data,
                                             int64_t size) {
  ERR_clear_error();
  if (size < 0 ||
      !EVP_DigestUpdate((EVP_MD_CTX *)(uintptr_t)value, data, (size_t)size))
    return scoop_crypto_fail("digest update failed");
  return scoop_crypto_ok(0);
}

ScoopCryptoResult scoop_crypto_digest_finish(uint64_t value) {
  ERR_clear_error();
  unsigned char digest[EVP_MAX_MD_SIZE];
  unsigned int size = 0;
  if (!EVP_DigestFinal_ex((EVP_MD_CTX *)(uintptr_t)value, digest, &size))
    return scoop_crypto_fail("digest finish failed");
  return scoop_crypto_bytes_copy(digest, size);
}

void scoop_crypto_digest_free(uint64_t value) {
  EVP_MD_CTX_free((EVP_MD_CTX *)(uintptr_t)value);
}

ScoopCryptoResult scoop_crypto_hmac(int algorithm, uint64_t secret,
                                    const void *data, int64_t size) {
  ERR_clear_error();
  const char *name = scoop_crypto_digest_name(algorithm);
  const ScoopCryptoBytes *key = (const ScoopCryptoBytes *)(uintptr_t)secret;
  unsigned char output[EVP_MAX_MD_SIZE];
  size_t count = 0;
  if (size < 0 || !name ||
      !EVP_Q_mac(NULL, "HMAC", NULL, name, NULL, key->data, key->size, data,
                 (size_t)size, output, sizeof(output), &count))
    return scoop_crypto_fail("HMAC failed");
  return scoop_crypto_bytes_copy(output, count);
}

int scoop_crypto_equal(const void *left, const void *right, int64_t count) {
  return count >= 0 && CRYPTO_memcmp(left, right, (size_t)count) == 0;
}

ScoopCryptoResult scoop_crypto_random(int64_t count) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  ScoopCryptoBytes *bytes =
      count < 0 ? NULL : scoop_crypto_bytes_new((size_t)count);
  if (!bytes)
    return scoop_crypto_fail("invalid count or native allocation failed");
  if (count && RAND_bytes_ex(NULL, bytes->data, bytes->size, 0) != 1) {
    ScoopCryptoResult error = scoop_crypto_fail("secure random failed");
    scoop_crypto_bytes_free((uintptr_t)bytes);
    return error;
  }
  return scoop_crypto_ok((uintptr_t)bytes);
}
