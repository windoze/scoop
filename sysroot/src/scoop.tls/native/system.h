#ifndef SCOOP_TLS_SYSTEM_H
#define SCOOP_TLS_SYSTEM_H

#include "tls.h"
#include <openssl/x509v3.h>

typedef void (*ScoopTlsWorkFunction)(void *);
typedef struct {
  void *pointer;
  uint64_t error;
} ScoopTlsWorkCreated;

struct ScoopTlsVerification {
  X509 *leaf;
  STACK_OF(X509) * chain;
  X509_STORE *store;
  X509_VERIFY_PARAM *parameters;
  ScoopTlsStep result;
  int completed;
};

ScoopTlsStep scoop_tls_work_failure(int status, int system_error,
                                    int64_t verify_code, const char *message);
void scoop_crypto_error_free(uint64_t error);
ScoopTlsStep scoop_tls_system_load(ScoopTlsContext *context);
int scoop_tls_system_verify(X509_STORE_CTX *check, void *unused);
void scoop_tls_verification_free(void *value);
ScoopTlsStep scoop_tls_verify_platform(ScoopTlsVerification *work);

#endif
