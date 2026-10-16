#include <openssl/err.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>
#include <string.h>

static X509 *certificate(EVP_PKEY *key) {
  X509 *cert = X509_new();
  X509_EXTENSION *san = NULL;
  if (!cert || !X509_set_version(cert, 2) ||
      !ASN1_INTEGER_set(X509_get_serialNumber(cert), 1) ||
      !ASN1_TIME_set_string(X509_getm_notBefore(cert), "20240101000000Z") ||
      !ASN1_TIME_set_string(X509_getm_notAfter(cert), "20360101000000Z") ||
      !X509_set_pubkey(cert, key))
    goto failed;
  X509_NAME *name = X509_get_subject_name(cert);
  if (!X509_NAME_add_entry_by_txt(name, "CN", MBSTRING_ASC,
                                  (const unsigned char *)"localhost", -1, -1,
                                  0) ||
      !X509_set_issuer_name(cert, name))
    goto failed;
  san = X509V3_EXT_conf_nid(NULL, NULL, NID_subject_alt_name, "DNS:localhost");
  if (!san || !X509_add_ext(cert, san, -1) ||
      !X509_sign(cert, key, EVP_sha256()))
    goto failed;
  X509_EXTENSION_free(san);
  return cert;
failed:
  X509_EXTENSION_free(san);
  X509_free(cert);
  return NULL;
}

static int handshake_step(SSL *ssl) {
  ERR_clear_error();
  int status = SSL_do_handshake(ssl);
  if (status == 1)
    return 1;
  int error = SSL_get_error(ssl, status);
  return error == SSL_ERROR_WANT_READ || error == SSL_ERROR_WANT_WRITE ? 0 : -1;
}

int m51_native_probe(void) {
  int result = 0;
  EVP_PKEY *key = NULL;
  X509 *cert = NULL;
  SSL_CTX *client_ctx = NULL, *server_ctx = NULL;
  SSL *client = NULL, *server = NULL;
  BIO *client_bio = NULL, *server_bio = NULL;
  if (!OPENSSL_init_ssl(OPENSSL_INIT_NO_LOAD_CONFIG, NULL))
    goto done;
  key = EVP_PKEY_Q_keygen(NULL, NULL, "RSA", (size_t)2048);
  if (!key || !(cert = certificate(key)))
    goto done;
  client_ctx = SSL_CTX_new(TLS_client_method());
  server_ctx = SSL_CTX_new(TLS_server_method());
  if (!client_ctx || !server_ctx)
    goto done;
  SSL_CTX_set_verify(client_ctx, SSL_VERIFY_PEER, NULL);
  if (!X509_STORE_add_cert(SSL_CTX_get_cert_store(client_ctx), cert) ||
      !SSL_CTX_use_certificate(server_ctx, cert) ||
      !SSL_CTX_use_PrivateKey(server_ctx, key))
    goto done;
  client = SSL_new(client_ctx);
  server = SSL_new(server_ctx);
  if (!client || !server || !SSL_set1_host(client, "localhost") ||
      !BIO_new_bio_pair(&client_bio, 4096, &server_bio, 4096))
    goto done;
  SSL_set_bio(client, client_bio, client_bio);
  SSL_set_bio(server, server_bio, server_bio);
  client_bio = server_bio = NULL;
  SSL_set_connect_state(client);
  SSL_set_accept_state(server);
  for (int i = 0; i < 100; ++i) {
    int a = handshake_step(client), b = handshake_step(server);
    if (a < 0 || b < 0)
      goto done;
    if (a == 1 && b == 1)
      break;
  }
  if (!SSL_is_init_finished(client) || !SSL_is_init_finished(server))
    goto done;
  char message[4] = {0};
  size_t count = 0;
  if (!SSL_write_ex(client, "tls", 3, &count) || count != 3 ||
      !SSL_read_ex(server, message, sizeof(message), &count) || count != 3 ||
      memcmp(message, "tls", 3))
    goto done;
  result = 42;
done:
  if (result != 42)
    ERR_print_errors_fp(stderr);
  BIO_free(client_bio);
  BIO_free(server_bio);
  SSL_free(client);
  SSL_free(server);
  SSL_CTX_free(client_ctx);
  SSL_CTX_free(server_ctx);
  X509_free(cert);
  EVP_PKEY_free(key);
  return result;
}
