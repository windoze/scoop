#include "keys.h"
#include <openssl/ec.h>

ScoopCryptoResult scoop_crypto_ecdsa_convert(const unsigned char *data,
                                             int64_t size, int to_raw) {
  ERR_clear_error();
  ECDSA_SIG *signature = NULL;
  ScoopCryptoBytes *bytes = NULL;
  int ok = 0;
  if (to_raw && size >= 0) {
    const unsigned char *cursor = data;
    signature = d2i_ECDSA_SIG(NULL, &cursor, (long)size);
    if (signature && cursor == data + size &&
        (bytes = scoop_crypto_bytes_new(64))) {
      ok =
          BN_bn2binpad(ECDSA_SIG_get0_r(signature), bytes->data, 32) == 32 &&
          BN_bn2binpad(ECDSA_SIG_get0_s(signature), bytes->data + 32, 32) == 32;
    }
  } else if (!to_raw && size == 64) {
    signature = ECDSA_SIG_new();
    BIGNUM *r = BN_bin2bn(data, 32, NULL);
    BIGNUM *s = BN_bin2bn(data + 32, 32, NULL);
    if (signature && r && s && ECDSA_SIG_set0(signature, r, s)) {
      int length = i2d_ECDSA_SIG(signature, NULL);
      if (length > 0 && (bytes = scoop_crypto_bytes_new((size_t)length))) {
        unsigned char *cursor = bytes->data;
        ok = i2d_ECDSA_SIG(signature, &cursor) == length;
      }
    } else {
      BN_free(r);
      BN_free(s);
    }
  }
  ScoopCryptoResult result = ok ? scoop_crypto_ok((uintptr_t)bytes)
                                : scoop_crypto_fail("invalid ES256 signature");
  if (!ok)
    scoop_crypto_bytes_free((uintptr_t)bytes);
  ECDSA_SIG_free(signature);
  return result;
}
