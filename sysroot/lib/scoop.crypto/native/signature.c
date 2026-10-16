#include "keys.h"

static EVP_MD_CTX *signature_context(EVP_PKEY *key, int algorithm, int sign) {
  int expected = algorithm < 2 ? 0 : algorithm == 2 ? 1 : 2;
  if (algorithm < 0 || algorithm > 3 || scoop_crypto_key_kind(key) != expected)
    return NULL;
  EVP_MD_CTX *ctx = EVP_MD_CTX_new();
  EVP_PKEY_CTX *pctx = NULL;
  if (!ctx)
    return NULL;
  const EVP_MD *md = algorithm == 3 ? NULL : EVP_sha256();
  int ok = sign ? EVP_DigestSignInit(ctx, &pctx, md, NULL, key)
                : EVP_DigestVerifyInit(ctx, &pctx, md, NULL, key);
  if (ok > 0 && algorithm == 0)
    ok = EVP_PKEY_CTX_set_rsa_padding(pctx, RSA_PKCS1_PADDING);
  if (ok > 0 && algorithm == 1) {
    ok = EVP_PKEY_CTX_set_rsa_padding(pctx, RSA_PKCS1_PSS_PADDING) > 0 &&
         EVP_PKEY_CTX_set_rsa_mgf1_md(pctx, EVP_sha256()) > 0 &&
         EVP_PKEY_CTX_set_rsa_pss_saltlen(pctx, 32) > 0;
  }
  if (ok <= 0) {
    EVP_MD_CTX_free(ctx);
    return NULL;
  }
  return ctx;
}

ScoopCryptoResult scoop_crypto_sign(int algorithm, uint64_t value,
                                    const unsigned char *data, int64_t count) {
  ERR_clear_error();
  EVP_MD_CTX *ctx =
      signature_context((EVP_PKEY *)(uintptr_t)value, algorithm, 1);
  ScoopCryptoBytes *bytes = NULL;
  size_t size = 0;
  if (!ctx || count < 0 ||
      EVP_DigestSign(ctx, NULL, &size, data, (size_t)count) <= 0 ||
      !(bytes = scoop_crypto_bytes_new(size)) ||
      EVP_DigestSign(ctx, bytes->data, &size, data, (size_t)count) <= 0) {
    ScoopCryptoResult result = scoop_crypto_fail("signature failed");
    EVP_MD_CTX_free(ctx);
    scoop_crypto_bytes_free((uintptr_t)bytes);
    return result;
  }
  bytes->size = size;
  EVP_MD_CTX_free(ctx);
  return scoop_crypto_ok((uintptr_t)bytes);
}

ScoopCryptoResult scoop_crypto_verify(int algorithm, uint64_t value,
                                      const unsigned char *data, int64_t count,
                                      const unsigned char *signature,
                                      int64_t size) {
  ERR_clear_error();
  EVP_MD_CTX *ctx =
      signature_context((EVP_PKEY *)(uintptr_t)value, algorithm, 0);
  if (!ctx || count < 0 || size < 0) {
    ScoopCryptoResult result =
        scoop_crypto_fail("signature verification initialization failed");
    EVP_MD_CTX_free(ctx);
    return result;
  }
  int valid =
      EVP_DigestVerify(ctx, signature, (size_t)size, data, (size_t)count);
  ScoopCryptoResult result =
      valid < 0 ? scoop_crypto_fail("signature verification failed")
                : scoop_crypto_ok((uint64_t)valid);
  if (valid == 0)
    ERR_clear_error();
  EVP_MD_CTX_free(ctx);
  return result;
}
