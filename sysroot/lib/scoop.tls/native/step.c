#include "tls.h"
#include <errno.h>

static ScoopTlsStep failure(SSL *ssl, int status, int system_error,
                            const char *message) {
  long verification = SSL_get_verify_result(ssl);
  if (verification != X509_V_OK)
    status = -3;
  ScoopCryptoResult error = scoop_crypto_fail(message);
  return (ScoopTlsStep){status, system_error, 0, verification, error.error};
}

ScoopTlsStep scoop_tls_step(uint64_t value, int operation, unsigned char *data,
                            int64_t length) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  SSL *ssl = session->ssl;
  ERR_clear_error();
  if (operation == 3 && SSL_pending(ssl) > 0)
    return failure(ssl, -4, 0,
                   "unconsumed TLS application data during shutdown");
  size_t count = 0;
  int result;
  errno = 0;
  switch (operation) {
  case 0:
    result = SSL_do_handshake(ssl);
    break;
  case 1:
    result = SSL_read_ex(ssl, data, (size_t)length, &count);
    break;
  case 2:
    result = SSL_write_ex(ssl, data, (size_t)length, &count);
    break;
  case 3:
    result = SSL_shutdown(ssl);
    break;
  default:
    return failure(ssl, -1, 0, "invalid TLS operation");
  }
  int saved_errno = errno;
  // SSL_shutdown(0) is a successful first half, not an SSL_get_error input.
  if (operation == 3 && result == 0)
    return (ScoopTlsStep){1, 0, 0, 0, 0};
  int error = SSL_get_error(ssl, result);
  if (error == SSL_ERROR_NONE)
    return (ScoopTlsStep){0, 0, (int64_t)count, 0, 0};
  if (error == SSL_ERROR_WANT_READ || error == SSL_ERROR_WANT_WRITE)
    return (ScoopTlsStep){error == SSL_ERROR_WANT_READ ? 1 : 2, 0, 0, 0, 0};
  if (error == SSL_ERROR_ZERO_RETURN)
    return (ScoopTlsStep){3, 0, 0, 0, 0};
  unsigned long code = ERR_peek_error();
  int status =
      ERR_GET_LIB(code) == ERR_LIB_SSL &&
              ERR_GET_REASON(code) == SSL_R_UNEXPECTED_EOF_WHILE_READING
          ? -2
          : -1;
  if (operation == 3 && ERR_GET_LIB(code) == ERR_LIB_SSL &&
      ERR_GET_REASON(code) == SSL_R_APPLICATION_DATA_AFTER_CLOSE_NOTIFY)
    status = -4;
  return failure(ssl, status, error == SSL_ERROR_SYSCALL ? saved_errno : 0,
                 "TLS protocol operation failed");
}
