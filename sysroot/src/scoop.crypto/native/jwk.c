#include "keys.h"
#include <limits.h>
#include <openssl/param_build.h>
#include <string.h>

static EVP_PKEY *import_parameters(int kind, const unsigned char *first,
                                   int first_size, const unsigned char *second,
                                   int second_size) {
  if (kind == 2)
    return first_size == 32 ? EVP_PKEY_new_raw_public_key_ex(NULL, "ED25519",
                                                             NULL, first, 32)
                            : NULL;
  OSSL_PARAM_BLD *builder = OSSL_PARAM_BLD_new();
  OSSL_PARAM *parameters = NULL;
  EVP_PKEY_CTX *ctx = NULL;
  EVP_PKEY *key = NULL;
  BIGNUM *n = NULL, *e = NULL;
  unsigned char point[65];
  int ok = builder != NULL;
  if (ok && kind == 0) {
    n = BN_bin2bn(first, first_size, NULL);
    e = BN_bin2bn(second, second_size, NULL);
    ok = n && e && OSSL_PARAM_BLD_push_BN(builder, OSSL_PKEY_PARAM_RSA_N, n) &&
         OSSL_PARAM_BLD_push_BN(builder, OSSL_PKEY_PARAM_RSA_E, e);
  } else if (ok && kind == 1 && first_size == 32 && second_size == 32) {
    point[0] = 4;
    memcpy(point + 1, first, 32);
    memcpy(point + 33, second, 32);
    ok = OSSL_PARAM_BLD_push_utf8_string(builder, OSSL_PKEY_PARAM_GROUP_NAME,
                                         "prime256v1", 0) &&
         OSSL_PARAM_BLD_push_octet_string(builder, OSSL_PKEY_PARAM_PUB_KEY,
                                          point, sizeof(point));
  } else {
    ok = 0;
  }
  if (ok && (parameters = OSSL_PARAM_BLD_to_param(builder)) &&
      (ctx =
           EVP_PKEY_CTX_new_from_name(NULL, kind == 0 ? "RSA" : "EC", NULL)) &&
      EVP_PKEY_fromdata_init(ctx) > 0)
    EVP_PKEY_fromdata(ctx, &key, EVP_PKEY_PUBLIC_KEY, parameters);
  EVP_PKEY_CTX_free(ctx);
  OSSL_PARAM_free(parameters);
  OSSL_PARAM_BLD_free(builder);
  BN_free(n);
  BN_free(e);
  return key;
}

ScoopCryptoResult scoop_crypto_jwk_import(int kind, const unsigned char *first,
                                          int64_t first_size,
                                          const unsigned char *second,
                                          int64_t second_size) {
  ERR_clear_error();
  EVP_PKEY *key = NULL;
  EVP_PKEY_CTX *ctx = NULL;
  int ok = kind >= 0 && kind <= 2 && first_size > 0 && first_size <= INT_MAX &&
           second_size >= 0 && second_size <= INT_MAX;
  if (ok)
    key = import_parameters(kind, first, (int)first_size, second,
                            (int)second_size);
  ok = key && (ctx = EVP_PKEY_CTX_new_from_pkey(NULL, key, NULL)) &&
       EVP_PKEY_public_check(ctx) > 0;
  ScoopCryptoResult result = ok ? scoop_crypto_ok((uintptr_t)key)
                                : scoop_crypto_fail("invalid JWK public key");
  EVP_PKEY_CTX_free(ctx);
  if (!ok)
    EVP_PKEY_free(key);
  return result;
}
