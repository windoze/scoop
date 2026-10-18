#include "tls.h"
#include <string.h>

static int select_alpn(SSL *ssl, const unsigned char **out, unsigned char *size,
                       const unsigned char *offered, unsigned int offered_size,
                       void *user) {
  (void)ssl;
  const ScoopTlsContext *ctx = user;
  for (unsigned int own = 0; own < ctx->alpn_size;) {
    unsigned int length = ctx->alpn[own++];
    for (unsigned int peer = 0; peer < offered_size;) {
      unsigned int candidate = offered[peer++];
      if (candidate == 0 || candidate > offered_size - peer)
        return SSL_TLSEXT_ERR_ALERT_FATAL;
      if (candidate == length &&
          memcmp(ctx->alpn + own, offered + peer, length) == 0) {
        *out = ctx->alpn + own;
        *size = (unsigned char)length;
        return SSL_TLSEXT_ERR_OK;
      }
      peer += candidate;
    }
    own += length;
  }
  return SSL_TLSEXT_ERR_ALERT_FATAL;
}

ScoopCryptoResult scoop_tls_context_new(int server, int minimum, int maximum,
                                        const unsigned char *alpn,
                                        int64_t alpn_size, uint64_t store) {
  ERR_clear_error();
  ScoopTlsContext *ctx = OPENSSL_zalloc(sizeof(*ctx));
  if (!ctx)
    return scoop_crypto_fail("TLS context allocation failed");
  atomic_init(&ctx->references, 1);
  ctx->ssl = SSL_CTX_new(server ? TLS_server_method() : TLS_client_method());
  int ok = ctx->ssl && SSL_CTX_set_min_proto_version(ctx->ssl, minimum) &&
           SSL_CTX_set_max_proto_version(ctx->ssl, maximum);
  if (ok && !server) {
    SSL_CTX_set_verify(ctx->ssl, SSL_VERIFY_PEER, NULL);
    SSL_CTX_set1_cert_store(ctx->ssl, (X509_STORE *)(uintptr_t)store);
  }
  if (ok && alpn_size > 0 && alpn_size <= 65535) {
    ctx->alpn = OPENSSL_memdup(alpn, (size_t)alpn_size);
    ok = ctx->alpn != NULL;
    ctx->alpn_size = (unsigned int)alpn_size;
    if (ok && server)
      SSL_CTX_set_alpn_select_cb(ctx->ssl, select_alpn, ctx);
    else if (ok)
      ok = SSL_CTX_set_alpn_protos(ctx->ssl, ctx->alpn, ctx->alpn_size) == 0;
  } else if (alpn_size != 0) {
    ok = 0;
  }
  if (!ok) {
    ScoopCryptoResult result = scoop_crypto_fail("TLS context setup failed");
    scoop_tls_context_free((uintptr_t)ctx);
    return result;
  }
  return scoop_crypto_ok((uintptr_t)ctx);
}

void scoop_tls_context_free(uint64_t value) {
  ScoopTlsContext *ctx = (ScoopTlsContext *)(uintptr_t)value;
  if (ctx && atomic_fetch_sub_explicit(&ctx->references, 1,
                                       memory_order_acq_rel) == 1) {
    SSL_CTX_free(ctx->ssl);
    OPENSSL_free(ctx->alpn);
    OPENSSL_free(ctx);
  }
}
