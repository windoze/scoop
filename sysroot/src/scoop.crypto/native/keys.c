#include "keys.h"
#include <string.h>

int scoop_crypto_key_kind(EVP_PKEY *key) {
  if (EVP_PKEY_is_a(key, "RSA") || EVP_PKEY_is_a(key, "RSA-PSS"))
    return 0;
  if (EVP_PKEY_is_a(key, "ED25519"))
    return 2;
  char group[64];
  size_t size = 0;
  if (EVP_PKEY_is_a(key, "EC") &&
      EVP_PKEY_get_utf8_string_param(key, OSSL_PKEY_PARAM_GROUP_NAME, group,
                                     sizeof(group), &size) &&
      (!strcmp(group, "prime256v1") || !strcmp(group, "P-256") ||
       !strcmp(group, "secp256r1")))
    return 1;
  return 3;
}

int scoop_crypto_key_algorithm(uint64_t value) {
  return scoop_crypto_key_kind((EVP_PKEY *)(uintptr_t)value);
}

ScoopCryptoResult scoop_crypto_key_generate(int algorithm, int bits) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  EVP_PKEY *key = NULL;
  switch (algorithm) {
  case 0:
    if (bits >= 2048 && bits % 256 == 0)
      key = EVP_PKEY_Q_keygen(NULL, NULL, "RSA", (size_t)bits);
    break;
  case 1:
    key = EVP_PKEY_Q_keygen(NULL, NULL, "EC", "prime256v1");
    break;
  case 2:
    key = EVP_PKEY_Q_keygen(NULL, NULL, "ED25519");
    break;
  default:
    break;
  }
  if (!key)
    return scoop_crypto_fail("key generation failed");
  return scoop_crypto_ok((uintptr_t)key);
}

ScoopCryptoResult scoop_crypto_public_key(uint64_t value) {
  ERR_clear_error();
  EVP_PKEY *original = (EVP_PKEY *)(uintptr_t)value, *key = NULL;
  OSSL_PARAM *parameters = NULL;
  EVP_PKEY_CTX *ctx =
      EVP_PKEY_CTX_new_from_name(NULL, EVP_PKEY_get0_type_name(original), NULL);
  int ok = ctx &&
           EVP_PKEY_todata(original, EVP_PKEY_PUBLIC_KEY, &parameters) > 0 &&
           EVP_PKEY_fromdata_init(ctx) > 0 &&
           EVP_PKEY_fromdata(ctx, &key, EVP_PKEY_PUBLIC_KEY, parameters) > 0;
  ScoopCryptoResult result =
      ok ? scoop_crypto_ok((uintptr_t)key)
         : scoop_crypto_fail("public key extraction failed");
  if (!ok)
    EVP_PKEY_free(key);
  EVP_PKEY_CTX_free(ctx);
  OSSL_PARAM_free(parameters);
  return result;
}

void scoop_crypto_key_free(uint64_t value) {
  EVP_PKEY_free((EVP_PKEY *)(uintptr_t)value);
}
