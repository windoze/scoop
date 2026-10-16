#include "tls.h"

int64_t scoop_tls_input_capacity(uint64_t value) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  return session->input_ended
             ? 0
             : (int64_t)BIO_ctrl_get_write_guarantee(session->network);
}

int64_t scoop_tls_pending_output(uint64_t value) {
  return (int64_t)BIO_ctrl_pending(
      ((ScoopTlsSession *)(uintptr_t)value)->network);
}

ScoopCryptoResult scoop_tls_ciphertext(uint64_t value, int input,
                                       unsigned char *data, int64_t count) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  ERR_clear_error();
  size_t size = 0;
  int ok = input ? BIO_write_ex(session->network, data, (size_t)count, &size)
                 : BIO_read_ex(session->network, data, (size_t)count, &size);
  if (ok || BIO_should_retry(session->network))
    return scoop_crypto_ok(size);
  return scoop_crypto_fail("TLS ciphertext transfer failed");
}

void scoop_tls_end_input(uint64_t value) {
  ScoopTlsSession *session = (ScoopTlsSession *)(uintptr_t)value;
  if (!session->input_ended) {
    session->input_ended = 1;
    (void)BIO_shutdown_wr(session->network);
  }
}
