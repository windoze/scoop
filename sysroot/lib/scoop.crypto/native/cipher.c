#include "common.h"
#include <limits.h>

static int update(EVP_CIPHER_CTX *ctx, unsigned char *output,
                  const unsigned char *input, size_t count) {
  while (count) {
    int step = count > INT_MAX ? INT_MAX : (int)count, written = 0;
    if (!EVP_CipherUpdate(ctx, output, &written, input, step))
      return 0;
    if (output) {
      if (written != step)
        return 0;
      output += written;
    }
    input += step;
    count -= (size_t)step;
  }
  return 1;
}

ScoopCryptoResult scoop_crypto_aead(int algorithm, int encrypt, uint64_t secret,
                                    const unsigned char *nonce,
                                    const unsigned char *aad, int64_t aad_size,
                                    const unsigned char *input,
                                    int64_t input_size,
                                    const unsigned char *tag) {
  ERR_clear_error();
  const ScoopCryptoBytes *key = (const ScoopCryptoBytes *)(uintptr_t)secret;
  const EVP_CIPHER *cipher =
      algorithm == 0 ? EVP_aes_128_gcm() : EVP_aes_256_gcm();
  if ((algorithm != 0 && algorithm != 1) ||
      key->size != (algorithm == 0 ? 16u : 32u) || aad_size < 0 ||
      input_size < 0 || input_size > INT64_MAX - 16)
    return scoop_crypto_fail("invalid AES-GCM parameters");
  EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
  ScoopCryptoBytes *output = scoop_crypto_bytes_new((size_t)input_size + 16);
  int final_size = 0;
  ScoopCryptoResult result;
  if (!ctx || !output ||
      !EVP_CipherInit_ex(ctx, cipher, NULL, NULL, NULL, encrypt) ||
      !EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, 12, NULL) ||
      !EVP_CipherInit_ex(ctx, NULL, NULL, key->data, nonce, encrypt) ||
      !update(ctx, NULL, aad, (size_t)aad_size) ||
      !update(ctx, output->data, input, (size_t)input_size))
    goto failed;
  if (!encrypt &&
      !EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, 16, (void *)tag))
    goto failed;
  if (EVP_CipherFinal_ex(ctx, output->data + input_size, &final_size) != 1) {
    if (encrypt)
      goto failed;
    ERR_clear_error();
    result = scoop_crypto_ok(0);
    goto done;
  }
  if (final_size != 0)
    goto failed;
  if (encrypt && !EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, 16,
                                      output->data + input_size))
    goto failed;
  output->size = (size_t)input_size + (encrypt ? 16 : 0);
  result = scoop_crypto_ok((uintptr_t)output);
  output = NULL;
  goto done;
failed:
  result = scoop_crypto_fail("AES-GCM failed");
done:
  EVP_CIPHER_CTX_free(ctx);
  scoop_crypto_bytes_free((uintptr_t)output);
  return result;
}
