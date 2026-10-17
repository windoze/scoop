#include "keys.h"
#include <ctype.h>
#include <limits.h>

int scoop_crypto_no_password(char *buffer, int size, int writing, void *user) {
  (void)buffer;
  (void)size;
  (void)writing;
  (void)user;
  return -1;
}

static int only_space(BIO *bio) {
  unsigned char byte;
  while (BIO_read(bio, &byte, 1) == 1)
    if (!isspace(byte))
      return 0;
  return 1;
}

ScoopCryptoResult scoop_crypto_key_import(int private_key, int pem,
                                          const unsigned char *data,
                                          int64_t count) {
  if (!scoop_crypto_begin())
    return scoop_crypto_fail("OpenSSL initialization failed");
  if (count <= 0 || count > INT_MAX)
    return scoop_crypto_fail("invalid key data length");
  BIO *bio = BIO_new_mem_buf(data, (int)count);
  if (!bio)
    return scoop_crypto_fail("key input allocation failed");
  EVP_PKEY *key = NULL;
  PKCS8_PRIV_KEY_INFO *info = NULL;
  const unsigned char *cursor = data;
  if (private_key) {
    info = pem ? PEM_read_bio_PKCS8_PRIV_KEY_INFO(
                     bio, NULL, scoop_crypto_no_password, NULL)
               : d2i_PKCS8_PRIV_KEY_INFO(NULL, &cursor, (long)count);
    if (info)
      key = EVP_PKCS82PKEY(info);
  } else {
    key = pem ? PEM_read_bio_PUBKEY(bio, NULL, scoop_crypto_no_password, NULL)
              : d2i_PUBKEY(NULL, &cursor, (long)count);
  }
  int complete = pem ? only_space(bio) : cursor == data + count;
  if (key && private_key && scoop_crypto_key_kind(key) == 3)
    complete = 0;
  ScoopCryptoResult result;
  if (!key || !complete) {
    result =
        scoop_crypto_fail("invalid key format, algorithm or trailing data");
    EVP_PKEY_free(key);
  } else
    result = scoop_crypto_ok((uintptr_t)key);
  PKCS8_PRIV_KEY_INFO_free(info);
  BIO_free(bio);
  return result;
}

ScoopCryptoResult scoop_crypto_bio_bytes(BIO *bio) {
  char *data = NULL;
  long count = BIO_get_mem_data(bio, &data);
  if (count < 0)
    return scoop_crypto_fail("memory output failed");
  return scoop_crypto_bytes_copy(data, (size_t)count);
}

ScoopCryptoResult scoop_crypto_key_export(uint64_t value, int private_key,
                                          int pem) {
  ERR_clear_error();
  EVP_PKEY *key = (EVP_PKEY *)(uintptr_t)value;
  BIO *bio = BIO_new(BIO_s_mem());
  PKCS8_PRIV_KEY_INFO *info = NULL;
  int ok = 0;
  if (bio) {
    if (private_key) {
      info = EVP_PKEY2PKCS8(key);
      if (info)
        ok = pem ? PEM_write_bio_PKCS8_PRIV_KEY_INFO(bio, info)
                 : i2d_PKCS8_PRIV_KEY_INFO_bio(bio, info);
    } else
      ok = pem ? PEM_write_bio_PUBKEY(bio, key) : i2d_PUBKEY_bio(bio, key);
  }
  ScoopCryptoResult result =
      ok ? scoop_crypto_bio_bytes(bio) : scoop_crypto_fail("key export failed");
  if (private_key && bio) {
    char *data = NULL;
    long size = BIO_get_mem_data(bio, &data);
    if (size > 0)
      OPENSSL_cleanse(data, (size_t)size);
  }
  PKCS8_PRIV_KEY_INFO_free(info);
  BIO_free(bio);
  return result;
}
