#ifndef SCOOP_CRYPTO_KEYS_H
#define SCOOP_CRYPTO_KEYS_H

#include "common.h"
#include <openssl/core_names.h>
#include <openssl/pem.h>
#include <openssl/rsa.h>

int scoop_crypto_no_password(char *buffer, int size, int writing, void *user);
int scoop_crypto_key_kind(EVP_PKEY *key);
ScoopCryptoResult scoop_crypto_bio_bytes(BIO *bio);

#endif
