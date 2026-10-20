#include "system.h"
#include <errno.h>

ScoopTlsStep scoop_tls_verify_platform(ScoopTlsVerification *work) {
  X509_STORE_CTX *check = X509_STORE_CTX_new();
  if (!check ||
      !X509_STORE_CTX_init(check, work->store, work->leaf, work->chain) ||
      !X509_VERIFY_PARAM_set1(X509_STORE_CTX_get0_param(check),
                              work->parameters)) {
    X509_STORE_CTX_free(check);
    return scoop_tls_work_failure(-3, 0, X509_V_ERR_OUT_OF_MEM,
                                  "system TLS verification setup failed");
  }
  errno = 0;
  int verified = X509_verify_cert(check);
  int saved_errno = errno;
  long code = X509_STORE_CTX_get_error(check);
  X509_STORE_CTX_free(check);
  if (verified == 1)
    return (ScoopTlsStep){0};
  return scoop_tls_work_failure(-3, saved_errno, code,
                                X509_verify_cert_error_string(code));
}
