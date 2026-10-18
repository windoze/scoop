#include "../../native/system.h"
#include <openssl/pem.h>

ScoopCryptoResult scoop_test_tls_short_certificate(uint64_t input,
                                                   uint64_t issuer,
                                                   uint64_t signing_key,
                                                   int expired) {
  ERR_clear_error();
  X509 *certificate = X509_dup((X509 *)(uintptr_t)input);
  X509 *root = (X509 *)(uintptr_t)issuer;
  BIO *output = BIO_new(BIO_s_mem());
  X509_EXTENSION *authority = NULL;
  int ok = certificate && output &&
           X509_set_issuer_name(certificate, X509_get_subject_name(root)) &&
           X509_gmtime_adj(X509_getm_notBefore(certificate),
                           expired ? -864000L : -86400L) &&
           X509_gmtime_adj(X509_getm_notAfter(certificate),
                           expired ? -86400L : 604800L);
  if (ok) {
    int index =
        X509_get_ext_by_NID(certificate, NID_authority_key_identifier, -1);
    if (index >= 0)
      X509_EXTENSION_free(X509_delete_ext(certificate, index));
    X509V3_CTX context;
    X509V3_set_ctx(&context, root, certificate, NULL, NULL, 0);
    authority = X509V3_EXT_conf_nid(
        NULL, &context, NID_authority_key_identifier, "keyid:always");
    ok = authority && X509_add_ext(certificate, authority, -1) &&
         X509_sign(certificate, (EVP_PKEY *)(uintptr_t)signing_key,
                   EVP_sha256()) &&
         PEM_write_bio_X509(output, certificate);
  }
  ScoopCryptoResult result;
  if (ok) {
    char *data = NULL;
    long count = BIO_get_mem_data(output, &data);
    result = scoop_crypto_bytes_copy(data, (size_t)count);
  } else {
    result =
        scoop_crypto_fail("short-lived system trust test certificate failed");
  }
  X509_EXTENSION_free(authority);
  X509_free(certificate);
  BIO_free(output);
  return result;
}
