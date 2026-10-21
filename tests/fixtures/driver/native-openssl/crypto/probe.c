#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <string.h>

int m51_native_probe(void) {
  static const unsigned char expected[32] = {
      0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40,
      0xde, 0x5d, 0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17,
      0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad};
  unsigned char hash[32], signature[64];
  size_t size = sizeof(signature);
  int result = 0;
  EVP_PKEY *key = NULL;
  EVP_MD_CTX *sign = NULL, *verify = NULL;
  if (!OPENSSL_init_crypto(OPENSSL_INIT_NO_LOAD_CONFIG, NULL))
    goto done;
  if (!EVP_Q_digest(NULL, "SHA256", NULL, "abc", 3, hash, NULL))
    goto done;
  if (CRYPTO_memcmp(hash, expected, sizeof(hash)))
    goto done;
  key = EVP_PKEY_Q_keygen(NULL, NULL, "ED25519");
  sign = EVP_MD_CTX_new();
  verify = EVP_MD_CTX_new();
  if (!key || !sign || !verify)
    goto done;
  if (EVP_DigestSignInit(sign, NULL, NULL, NULL, key) != 1)
    goto done;
  if (EVP_DigestSign(sign, signature, &size, hash, sizeof(hash)) != 1)
    goto done;
  if (EVP_DigestVerifyInit(verify, NULL, NULL, NULL, key) != 1)
    goto done;
  if (EVP_DigestVerify(verify, signature, size, hash, sizeof(hash)) != 1)
    goto done;
  signature[0] ^= 1;
  if (EVP_DigestVerify(verify, signature, size, hash, sizeof(hash)) != 0)
    goto done;
  result = 42;
done:
  EVP_MD_CTX_free(sign);
  EVP_MD_CTX_free(verify);
  EVP_PKEY_free(key);
  return result;
}
