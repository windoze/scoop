#ifndef SCOOP_TLS_H
#define SCOOP_TLS_H

#include "../../scoop.crypto/native/common.h"
#include <openssl/ssl.h>
#include <stdatomic.h>

typedef struct {
  SSL_CTX *ssl;
  unsigned char *alpn;
  unsigned int alpn_size;
  atomic_uint references;
} ScoopTlsContext;

typedef struct ScoopTlsVerification ScoopTlsVerification;
typedef enum {
  SCOOP_TLS_VERIFY_EMPTY,
  SCOOP_TLS_VERIFY_PENDING,
  SCOOP_TLS_VERIFY_WORKING,
  SCOOP_TLS_VERIFY_READY
} ScoopTlsVerifyState;

typedef struct {
  int status, system_error;
  int64_t count, verify_code;
  uint64_t error;
} ScoopTlsStep;

typedef struct {
  SSL *ssl;
  BIO *network;
  ScoopTlsContext *context;
  int input_ended;
  ScoopTlsVerification *verification;
  ScoopTlsVerifyState verification_state;
  ScoopTlsStep verification_result;
} ScoopTlsSession;

void scoop_tls_context_free(uint64_t value);
void scoop_tls_session_free(uint64_t value);

#endif
