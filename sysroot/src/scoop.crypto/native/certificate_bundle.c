#include "keys.h"
#include <limits.h>
#include <openssl/x509.h>
#include <string.h>

static int pem_space(unsigned char byte) {
  return byte == ' ' || byte == '\t' || byte == '\r' || byte == '\n' ||
         byte == '\v' || byte == '\f';
}

ScoopCryptoResult
scoop_crypto_certificate_bundle_parse(const unsigned char *data,
                                      int64_t count) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  if (count <= 0 || count > INT_MAX)
    return scoop_crypto_fail("invalid certificate bundle length");
  BIO *bio = BIO_new_mem_buf(data, (int)count);
  STACK_OF(X509) *certificates = sk_X509_new_null();
  const char *failure = "certificate bundle allocation failed";
  if (!bio || !certificates)
    goto failed;
  size_t offset = 0;
  while (offset < (size_t)count) {
    while (offset < (size_t)count && pem_space(data[offset]))
      ++offset;
    if (offset == (size_t)count)
      break;
    static const char begin[] = "-----BEGIN CERTIFICATE-----";
    failure = "unexpected data in certificate bundle";
    if ((size_t)count - offset < sizeof(begin) - 1 ||
        memcmp(data + offset, begin, sizeof(begin) - 1))
      goto failed;
    size_t line_end = offset + sizeof(begin) - 1;
    while (line_end < (size_t)count &&
           (data[line_end] == ' ' || data[line_end] == '\t'))
      ++line_end;
    if (line_end == (size_t)count ||
        (data[line_end] != '\r' && data[line_end] != '\n'))
      goto failed;
    failure = "invalid PEM certificate in bundle";
    if (BIO_seek(bio, (long)offset) < 0)
      goto failed;
    X509 *certificate =
        PEM_read_bio_X509(bio, NULL, scoop_crypto_no_password, NULL);
    if (!certificate)
      goto failed;
    long position = BIO_tell(bio);
    if (position <= (long)offset || position > count ||
        !sk_X509_push(certificates, certificate)) {
      X509_free(certificate);
      goto failed;
    }
    offset = (size_t)position;
  }
  failure = "empty certificate bundle";
  if (!sk_X509_num(certificates))
    goto failed;
  BIO_free(bio);
  return scoop_crypto_ok((uintptr_t)certificates);

failed:
  {
    ScoopCryptoResult result = scoop_crypto_fail(failure);
    sk_X509_pop_free(certificates, X509_free);
    BIO_free(bio);
    return result;
  }
}

int32_t scoop_crypto_certificate_bundle_count(uint64_t handle) {
  return sk_X509_num((STACK_OF(X509) *)(uintptr_t)handle);
}

ScoopCryptoResult scoop_crypto_certificate_bundle_get(uint64_t handle,
                                                      int32_t index) {
  ERR_clear_error();
  X509 *certificate = sk_X509_value((STACK_OF(X509) *)(uintptr_t)handle, index);
  if (!certificate || !X509_up_ref(certificate))
    return scoop_crypto_fail("cannot retain bundle certificate");
  return scoop_crypto_ok((uintptr_t)certificate);
}

void scoop_crypto_certificate_bundle_free(uint64_t handle) {
  sk_X509_pop_free((STACK_OF(X509) *)(uintptr_t)handle, X509_free);
}
