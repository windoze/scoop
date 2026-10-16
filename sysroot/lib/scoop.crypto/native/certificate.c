#include "keys.h"
#include <ctype.h>
#include <limits.h>
#include <openssl/x509v3.h>

ScoopCryptoResult scoop_crypto_certificate_import(int pem,
                                                  const unsigned char *data,
                                                  int64_t count) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  if (count <= 0 || count > INT_MAX)
    return scoop_crypto_fail("invalid certificate length");
  BIO *bio = BIO_new_mem_buf(data, (int)count);
  if (!bio)
    return scoop_crypto_fail("certificate input allocation failed");
  const unsigned char *cursor = data;
  X509 *certificate =
      pem ? PEM_read_bio_X509(bio, NULL, scoop_crypto_no_password, NULL)
          : d2i_X509(NULL, &cursor, (long)count);
  int complete = pem || cursor == data + count;
  if (pem) {
    unsigned char byte;
    while (BIO_read(bio, &byte, 1) == 1)
      if (!isspace(byte))
        complete = 0;
  }
  ScoopCryptoResult result;
  if (!certificate || !complete) {
    result = scoop_crypto_fail("invalid certificate or trailing data");
    X509_free(certificate);
  } else
    result = scoop_crypto_ok((uintptr_t)certificate);
  BIO_free(bio);
  return result;
}

ScoopCryptoResult scoop_crypto_certificate_export(uint64_t value, int pem) {
  ERR_clear_error();
  X509 *certificate = (X509 *)(uintptr_t)value;
  BIO *bio = BIO_new(BIO_s_mem());
  int ok = bio && (pem ? PEM_write_bio_X509(bio, certificate)
                       : i2d_X509_bio(bio, certificate));
  ScoopCryptoResult result =
      ok ? scoop_crypto_bio_bytes(bio)
         : scoop_crypto_fail("certificate export failed");
  BIO_free(bio);
  return result;
}

ScoopCryptoResult scoop_crypto_certificate_text(uint64_t value, int field) {
  ERR_clear_error();
  X509 *certificate = (X509 *)(uintptr_t)value;
  BIO *bio = BIO_new(BIO_s_mem());
  int ok = 0;
  if (bio) {
    if (field == 2)
      ok = i2a_ASN1_INTEGER(bio, X509_get_serialNumber(certificate)) >= 0;
    else
      ok = X509_NAME_print_ex(bio,
                              field == 0 ? X509_get_subject_name(certificate)
                                         : X509_get_issuer_name(certificate),
                              0, XN_FLAG_RFC2253) >= 0;
  }
  ScoopCryptoResult result =
      ok ? scoop_crypto_bio_bytes(bio)
         : scoop_crypto_fail("certificate name export failed");
  BIO_free(bio);
  return result;
}

ScoopCryptoResult scoop_crypto_certificate_time(uint64_t value, int after) {
  ERR_clear_error();
  X509 *certificate = (X509 *)(uintptr_t)value;
  const ASN1_TIME *time = after ? X509_get0_notAfter(certificate)
                                : X509_get0_notBefore(certificate);
  ASN1_TIME *epoch = ASN1_TIME_set(NULL, 0);
  int days = 0, seconds = 0;
  int ok = epoch && ASN1_TIME_diff(&days, &seconds, epoch, time);
  ScoopCryptoResult result =
      ok ? scoop_crypto_ok((uint64_t)((int64_t)days * 86400 + seconds))
         : scoop_crypto_fail("invalid certificate time");
  ASN1_TIME_free(epoch);
  return result;
}

ScoopCryptoResult scoop_crypto_certificate_key(uint64_t value) {
  ERR_clear_error();
  EVP_PKEY *key = X509_get_pubkey((X509 *)(uintptr_t)value);
  return key ? scoop_crypto_ok((uintptr_t)key)
             : scoop_crypto_fail("certificate public key unavailable");
}

ScoopCryptoResult scoop_crypto_certificate_signature(uint64_t value,
                                                     uint64_t key) {
  ERR_clear_error();
  int result =
      X509_verify((X509 *)(uintptr_t)value, (EVP_PKEY *)(uintptr_t)key);
  if (result < 0)
    return scoop_crypto_fail("certificate signature check failed");
  if (result == 0)
    ERR_clear_error();
  return scoop_crypto_ok((uint64_t)result);
}

void scoop_crypto_certificate_free(uint64_t value) {
  X509_free((X509 *)(uintptr_t)value);
}
