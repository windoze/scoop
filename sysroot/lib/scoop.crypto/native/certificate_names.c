#include "common.h"
#include <openssl/x509v3.h>

ScoopCryptoResult scoop_crypto_certificate_names(uint64_t value) {
  ERR_clear_error();
  int critical = -1;
  GENERAL_NAMES *names = X509_get_ext_d2i(
      (X509 *)(uintptr_t)value, NID_subject_alt_name, &critical, NULL);
  if (!names && critical == -1)
    names = sk_GENERAL_NAME_new_null();
  return names ? scoop_crypto_ok((uintptr_t)names)
               : scoop_crypto_fail("invalid subject alternative names");
}

int scoop_crypto_names_count(uint64_t value) {
  return sk_GENERAL_NAME_num((GENERAL_NAMES *)(uintptr_t)value);
}

int scoop_crypto_name_kind(uint64_t value, int index) {
  GENERAL_NAME *name =
      sk_GENERAL_NAME_value((GENERAL_NAMES *)(uintptr_t)value, index);
  if (!name)
    return -1;
  if (name->type == GEN_DNS)
    return 0;
  if (name->type == GEN_IPADD)
    return 1;
  return 2;
}

ScoopCryptoResult scoop_crypto_name_bytes(uint64_t value, int index) {
  ERR_clear_error();
  GENERAL_NAME *name =
      sk_GENERAL_NAME_value((GENERAL_NAMES *)(uintptr_t)value, index);
  if (!name || (name->type != GEN_DNS && name->type != GEN_IPADD))
    return scoop_crypto_fail("invalid alternative name index");
  ASN1_STRING *text =
      name->type == GEN_DNS ? name->d.dNSName : name->d.iPAddress;
  int count = ASN1_STRING_length(text);
  if (count < 0 || (name->type == GEN_IPADD && count != 4 && count != 16))
    return scoop_crypto_fail("invalid alternative name data");
  return scoop_crypto_bytes_copy(ASN1_STRING_get0_data(text), (size_t)count);
}

void scoop_crypto_names_free(uint64_t value) {
  GENERAL_NAMES_free((GENERAL_NAMES *)(uintptr_t)value);
}
