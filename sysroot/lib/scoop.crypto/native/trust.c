#include "common.h"
#include <limits.h>
#include <openssl/x509v3.h>
#include <time.h>

typedef struct {
  int64_t code;
  int depth;
  uint64_t error;
} ScoopCertificateCheck;

ScoopCryptoResult scoop_crypto_store_new(void) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  X509_STORE *store = X509_STORE_new();
  return store ? scoop_crypto_ok((uintptr_t)store)
               : scoop_crypto_fail("trust store allocation failed");
}

ScoopCryptoResult scoop_crypto_store_add(uint64_t value, uint64_t certificate) {
  ERR_clear_error();
  if (!X509_STORE_add_cert((X509_STORE *)(uintptr_t)value,
                           (X509 *)(uintptr_t)certificate))
    return scoop_crypto_fail("trust anchor insertion failed");
  return scoop_crypto_ok(0);
}

void scoop_crypto_store_free(uint64_t value) {
  X509_STORE_free((X509_STORE *)(uintptr_t)value);
}

ScoopCertificateCheck
scoop_crypto_certificate_verify(uint64_t leaf, uint64_t store,
                                const uint64_t *intermediates, int64_t count,
                                int64_t time, int purpose, int name_kind,
                                const unsigned char *name, int64_t name_size) {
  ERR_clear_error();
  STACK_OF(X509) *chain = sk_X509_new_null();
  X509_STORE_CTX *ctx = X509_STORE_CTX_new();
  ScoopCertificateCheck result = {0, 0, 0};
  if (!chain || !ctx || count < 0 || count > INT_MAX || name_size < 0 ||
      name_size > INT_MAX)
    goto failed;
  for (int64_t i = 0; i < count; ++i)
    if (!sk_X509_push(chain, (X509 *)(uintptr_t)intermediates[i]))
      goto failed;
  if (!X509_STORE_CTX_init(ctx, (X509_STORE *)(uintptr_t)store,
                           (X509 *)(uintptr_t)leaf, chain))
    goto failed;
  int usage = purpose == 0   ? X509_PURPOSE_SSL_SERVER
              : purpose == 1 ? X509_PURPOSE_SSL_CLIENT
                             : X509_PURPOSE_ANY;
  if (purpose < 0 || purpose > 2 || !X509_STORE_CTX_set_purpose(ctx, usage))
    goto failed;
  X509_VERIFY_PARAM *parameters = X509_STORE_CTX_get0_param(ctx);
  X509_VERIFY_PARAM_set_time(parameters, (time_t)time);
  X509_VERIFY_PARAM_set_hostflags(parameters,
                                  X509_CHECK_FLAG_NO_PARTIAL_WILDCARDS);
  if (name_kind == 0 && !X509_VERIFY_PARAM_set1_host(
                            parameters, (const char *)name, (size_t)name_size))
    goto failed;
  if (name_kind == 1 &&
      !X509_VERIFY_PARAM_set1_ip(parameters, name, (size_t)name_size))
    goto failed;
  int verified = X509_verify_cert(ctx);
  if (verified < 0)
    goto failed;
  if (!verified) {
    result.code = X509_STORE_CTX_get_error(ctx);
    result.depth = X509_STORE_CTX_get_error_depth(ctx);
    ERR_clear_error();
  }
  goto done;
failed:
  result.error = scoop_crypto_fail("certificate validation failed").error;
done:
  X509_STORE_CTX_free(ctx);
  sk_X509_free(chain);
  return result;
}

const char *scoop_crypto_verify_message(int64_t code) {
  return X509_verify_cert_error_string(code);
}
