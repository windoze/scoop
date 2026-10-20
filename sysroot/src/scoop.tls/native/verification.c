#include "system.h"

static ScoopTlsVerification *prepare_verification(X509_STORE_CTX *source,
                                                  ScoopTlsContext *context) {
  ScoopTlsVerification *work = OPENSSL_zalloc(sizeof(*work));
  if (!work)
    return NULL;
  X509 *leaf = X509_STORE_CTX_get0_cert(source);
  if (!X509_up_ref(leaf))
    goto failed;
  work->leaf = leaf;
  STACK_OF(X509) *untrusted = X509_STORE_CTX_get0_untrusted(source);
  work->chain = untrusted ? X509_chain_up_ref(untrusted) : sk_X509_new_null();
  work->parameters = X509_VERIFY_PARAM_new();
  X509_STORE *store = SSL_CTX_get_cert_store(context->ssl);
  if (!work->chain || !work->parameters || !X509_STORE_up_ref(store))
    goto failed;
  work->store = store;
  if (!X509_VERIFY_PARAM_set1(work->parameters,
                              X509_STORE_CTX_get0_param(source)))
    goto failed;
  return work;
failed:
  scoop_tls_verification_free(work);
  return NULL;
}

int scoop_tls_system_verify(X509_STORE_CTX *check, void *unused) {
  (void)unused;
  SSL *ssl =
      X509_STORE_CTX_get_ex_data(check, SSL_get_ex_data_X509_STORE_CTX_idx());
  ScoopTlsSession *session = SSL_get_app_data(ssl);
  if (session->verification_state == SCOOP_TLS_VERIFY_READY) {
    session->verification_state = SCOOP_TLS_VERIFY_EMPTY;
    int failed = session->verification_result.error != 0;
    X509_STORE_CTX_set_error(check, failed ? X509_V_ERR_APPLICATION_VERIFICATION
                                           : X509_V_OK);
    return !failed;
  }
  if (session->verification_state == SCOOP_TLS_VERIFY_EMPTY) {
    session->verification = prepare_verification(check, session->context);
    if (!session->verification) {
      session->verification_result =
          scoop_tls_work_failure(-3, 0, X509_V_ERR_OUT_OF_MEM,
                                 "TLS verification work allocation failed");
      X509_STORE_CTX_set_error(check, X509_V_ERR_OUT_OF_MEM);
      return 0;
    }
    session->verification_state = SCOOP_TLS_VERIFY_PENDING;
  }
  (void)SSL_set_retry_verify(ssl);
  return 1;
}

ScoopTlsWorkCreated scoop_tls_verification_take(uint64_t value) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  ERR_clear_error();
  if (session->verification_state != SCOOP_TLS_VERIFY_PENDING)
    return (ScoopTlsWorkCreated){
        NULL, scoop_crypto_fail("TLS has no pending verification work").error};
  ScoopTlsVerification *work = session->verification;
  session->verification = NULL;
  session->verification_state = SCOOP_TLS_VERIFY_WORKING;
  return (ScoopTlsWorkCreated){work, 0};
}

void scoop_tls_verification_run(void *value) {
  ScoopTlsVerification *work = value;
  ERR_clear_error();
  work->result = scoop_tls_verify_platform(work);
  work->completed = 1;
}

ScoopCryptoResult scoop_tls_verification_complete(uint64_t value,
                                                  ScoopTlsVerification *work) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  ERR_clear_error();
  if (!work->completed ||
      session->verification_state != SCOOP_TLS_VERIFY_WORKING)
    return scoop_crypto_fail(
        "TLS verification result is not ready for transfer");
  session->verification_result = work->result;
  work->result.error = 0;
  work->completed = 0;
  session->verification_state = SCOOP_TLS_VERIFY_READY;
  return scoop_crypto_ok(0);
}

void scoop_tls_verification_free(void *value) {
  ScoopTlsVerification *work = value;
  if (work) {
    X509_free(work->leaf);
    sk_X509_pop_free(work->chain, X509_free);
    X509_STORE_free(work->store);
    X509_VERIFY_PARAM_free(work->parameters);
    if (work->result.error)
      scoop_crypto_error_free(work->result.error);
    OPENSSL_free(work);
  }
}

ScoopTlsWorkFunction scoop_tls_verification_executor(void) {
  return scoop_tls_verification_run;
}
ScoopTlsWorkFunction scoop_tls_verification_releaser(void) {
  return scoop_tls_verification_free;
}
