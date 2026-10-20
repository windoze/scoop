#include "system.h"
#include <openssl/x509v3.h>

ScoopCryptoResult scoop_tls_session_new(uint64_t value, int server,
                                        int name_kind,
                                        const unsigned char *name,
                                        int64_t name_size) {
  ERR_clear_error();
  ScoopTlsContext *ctx = (ScoopTlsContext *)(uintptr_t)value;
  ScoopTlsSession *session = OPENSSL_zalloc(sizeof(*session));
  if (!session)
    return scoop_crypto_fail("TLS session allocation failed");
  session->context = ctx;
  atomic_fetch_add_explicit(&ctx->references, 1, memory_order_relaxed);
  session->ssl = SSL_new(ctx->ssl);
  BIO *internal = NULL;
  int ok = session->ssl &&
           BIO_new_bio_pair(&internal, 16384, &session->network, 16384);
  if (ok) {
    SSL_set_app_data(session->ssl, session);
    SSL_set_bio(session->ssl, internal, internal);
    SSL_set_mode(session->ssl, SSL_MODE_ENABLE_PARTIAL_WRITE);
    if (server) {
      SSL_set_accept_state(session->ssl);
    } else {
      SSL_set_connect_state(session->ssl);
      X509_VERIFY_PARAM *param = SSL_get0_param(session->ssl);
      X509_VERIFY_PARAM_set_hostflags(param,
                                      X509_CHECK_FLAG_NO_PARTIAL_WILDCARDS);
      if (name_kind == 0 && name_size > 0) {
        char *dns = OPENSSL_strndup((const char *)name, (size_t)name_size);
        ok = dns && SSL_set_tlsext_host_name(session->ssl, dns) &&
             SSL_set1_host(session->ssl, dns);
        OPENSSL_free(dns);
      } else {
        ok = name_kind == 1 && (name_size == 4 || name_size == 16) &&
             X509_VERIFY_PARAM_set1_ip(param, name, (size_t)name_size);
      }
    }
  }
  if (!ok) {
    ScoopCryptoResult result = scoop_crypto_fail("TLS session setup failed");
    scoop_tls_session_free((uintptr_t)session);
    return result;
  }
  return scoop_crypto_ok((uintptr_t)session);
}

void scoop_tls_session_free(uint64_t value) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  if (session) {
    scoop_tls_verification_free(session->verification);
    if (session->verification_result.error)
      scoop_crypto_error_free(session->verification_result.error);
    SSL_free(session->ssl);
    BIO_free(session->network);
    scoop_tls_context_free((uintptr_t)session->context);
    OPENSSL_free(session);
  }
}

int scoop_tls_version(uint64_t value) {
  SSL *ssl = ((ScoopTlsSession *)(uintptr_t)value)->ssl;
  return SSL_is_init_finished(ssl) ? SSL_version(ssl) : 0;
}

ScoopCryptoResult scoop_tls_alpn(uint64_t value) {
  const unsigned char *protocol = NULL;
  unsigned int size = 0;
  ERR_clear_error();
  SSL_get0_alpn_selected(((ScoopTlsSession *)(uintptr_t)value)->ssl, &protocol,
                         &size);
  return scoop_crypto_bytes_copy(protocol, size);
}
