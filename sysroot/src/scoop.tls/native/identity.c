#include "tls.h"

typedef struct {
  EVP_PKEY *key;
  X509 *leaf;
  STACK_OF(X509) * chain;
} ScoopTlsIdentity;

void scoop_tls_identity_free(uint64_t value) {
  ScoopTlsIdentity *identity = (ScoopTlsIdentity *)(uintptr_t)value;
  if (identity) {
    EVP_PKEY_free(identity->key);
    X509_free(identity->leaf);
    sk_X509_pop_free(identity->chain, X509_free);
    OPENSSL_free(identity);
  }
}

ScoopCryptoResult scoop_tls_identity_new(uint64_t key,
                                         const uint64_t *certificates,
                                         int64_t count) {
  ERR_clear_error();
  if (!key || !certificates || count <= 0)
    return scoop_crypto_fail(
        "TLS identity requires a key and certificate chain");
  ScoopTlsIdentity *identity = OPENSSL_zalloc(sizeof(*identity));
  if (!identity)
    return scoop_crypto_fail("TLS identity allocation failed");
  identity->chain = sk_X509_new_null();
  int ok = identity->chain != NULL;
  if (ok && (ok = EVP_PKEY_up_ref((EVP_PKEY *)(uintptr_t)key)))
    identity->key = (EVP_PKEY *)(uintptr_t)key;
  if (ok && (ok = X509_up_ref((X509 *)(uintptr_t)certificates[0])))
    identity->leaf = (X509 *)(uintptr_t)certificates[0];
  if (ok)
    ok = X509_check_private_key(identity->leaf, identity->key);
  for (int64_t index = 1; ok && index < count; ++index) {
    X509 *certificate = (X509 *)(uintptr_t)certificates[index];
    ok = X509_up_ref(certificate);
    if (ok && !sk_X509_push(identity->chain, certificate)) {
      X509_free(certificate);
      ok = 0;
    }
  }
  if (!ok) {
    ScoopCryptoResult failure = scoop_crypto_fail("TLS identity setup failed");
    scoop_tls_identity_free((uintptr_t)identity);
    return failure;
  }
  return scoop_crypto_ok((uintptr_t)identity);
}

ScoopCryptoResult scoop_tls_context_identity(uint64_t context, uint64_t value) {
  ERR_clear_error();
  SSL_CTX *ctx = ((ScoopTlsContext *)(uintptr_t)context)->ssl;
  const ScoopTlsIdentity *identity = (ScoopTlsIdentity *)(uintptr_t)value;
  int ok = SSL_CTX_use_cert_and_key(ctx, identity->leaf, identity->key,
                                    identity->chain, 1);
  return ok ? scoop_crypto_ok(0)
            : scoop_crypto_fail("TLS certificate and private key setup failed");
}
