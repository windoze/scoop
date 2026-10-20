#include "system.h"

typedef struct {
  ScoopTlsContext *context;
  ScoopTlsStep result;
  int completed;
} SystemContextWork;

ScoopTlsStep scoop_tls_work_failure(int status, int system_error,
                                    int64_t verify_code, const char *message) {
  return (ScoopTlsStep){status, system_error, 0, verify_code,
                        scoop_crypto_fail(message).error};
}

ScoopTlsWorkCreated scoop_tls_system_context_new(uint64_t value) {
  ERR_clear_error();
  SystemContextWork *work = OPENSSL_zalloc(sizeof(*work));
  if (!work)
    return (ScoopTlsWorkCreated){
        NULL,
        scoop_crypto_fail("system TLS context work allocation failed").error};
  work->context = (ScoopTlsContext *)(uintptr_t)value;
  return (ScoopTlsWorkCreated){work, 0};
}

void scoop_tls_system_context_run(void *value) {
  SystemContextWork *work = value;
  ERR_clear_error();
  work->result = scoop_tls_system_load(work->context);
  if (!work->result.error) {
    SSL_CTX_set_cert_verify_callback(work->context->ssl,
                                     scoop_tls_system_verify, NULL);
  }
  work->completed = 1;
}

ScoopTlsStep scoop_tls_system_context_result(SystemContextWork *work) {
  if (!work->completed) {
    ERR_clear_error();
    return scoop_tls_work_failure(-5, 0, 0, "system TLS work is incomplete");
  }
  ScoopTlsStep result = work->result;
  work->result.error = 0;
  return result;
}

uint64_t scoop_tls_system_context_take(SystemContextWork *work) {
  ScoopTlsContext *context = work->context;
  work->context = NULL;
  return (uintptr_t)context;
}

void scoop_tls_system_context_free(void *value) {
  SystemContextWork *work = value;
  if (work) {
    scoop_tls_context_free((uintptr_t)work->context);
    if (work->result.error)
      scoop_crypto_error_free(work->result.error);
    OPENSSL_free(work);
  }
}

ScoopTlsWorkFunction scoop_tls_system_context_executor(void) {
  return scoop_tls_system_context_run;
}
ScoopTlsWorkFunction scoop_tls_system_context_releaser(void) {
  return scoop_tls_system_context_free;
}
