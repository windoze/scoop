#include "system.h"
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

static ScoopTlsStep source_failure(const char *path, int error) {
  char message[256];
  snprintf(message, sizeof(message), "system TLS trust source %s: %s", path,
           error ? strerror(error) : "invalid CA data");
  return scoop_tls_work_failure(-5, error, 0, message);
}

static int load_file(X509_STORE *store, const char *path, int optional,
                     ScoopTlsStep *failure) {
  struct stat info;
  errno = 0;
  if (stat(path, &info) != 0) {
    int error = errno;
    if (optional && error == ENOENT)
      return 0;
    *failure = source_failure(path, error);
    return -1;
  }
  errno = 0;
  if (!X509_STORE_load_file(store, path)) {
    *failure = source_failure(path, errno);
    return -1;
  }
  return 1;
}

static int certificate_name(const char *name) {
  if (strlen(name) < 10 || name[8] != '.')
    return 0;
  for (int i = 0; i < 8; ++i)
    if (!((name[i] >= '0' && name[i] <= '9') ||
          (name[i] >= 'a' && name[i] <= 'f')))
      return 0;
  if (name[9] == '0' && name[10] != '\0')
    return 0;
  unsigned number = 0;
  for (size_t i = 9; name[i]; ++i) {
    if (name[i] < '0' || name[i] > '9')
      return 0;
    unsigned digit = (unsigned)(name[i] - '0');
    if (number > ((unsigned)INT_MAX - digit) / 10)
      return 0;
    number = number * 10 + digit;
  }
  return 1;
}

static int load_entry(X509_STORE *store, const char *directory,
                      const char *name, ScoopTlsStep *failure) {
  size_t prefix = strlen(directory), suffix = strlen(name);
  if (prefix > SIZE_MAX - suffix - 2) {
    *failure = scoop_tls_work_failure(-5, ENAMETOOLONG, 0,
                                      "system CA certificate path is too long");
    return -1;
  }
  char *path = OPENSSL_malloc(prefix + suffix + 2);
  if (!path) {
    *failure = scoop_tls_work_failure(-5, 0, 0, "CA path allocation failed");
    return -1;
  }
  snprintf(path, prefix + suffix + 2, "%s/%s", directory, name);
  int loaded = load_file(store, path, 0, failure);
  OPENSSL_free(path);
  return loaded;
}

static int load_directory(X509_STORE *store, const char *path, int optional,
                          ScoopTlsStep *failure) {
  errno = 0;
  DIR *directory = opendir(path);
  if (!directory) {
    int error = errno;
    if (optional && error == ENOENT)
      return 0;
    *failure = source_failure(path, error);
    return -1;
  }
  int loaded = 0;
  while (true) {
    errno = 0;
    struct dirent *entry = readdir(directory);
    if (!entry) {
      if (errno) {
        *failure = source_failure(path, errno);
        loaded = -1;
      }
      break;
    }
    if (certificate_name(entry->d_name)) {
      if (load_entry(store, path, entry->d_name, failure) < 0) {
        loaded = -1;
        break;
      }
      loaded = 1;
    }
  }
  if (closedir(directory) != 0 && loaded >= 0) {
    *failure = source_failure(path, errno);
    loaded = -1;
  }
  return loaded;
}

static int system_files(X509_STORE *store, ScoopTlsStep *failure) {
  const char *configured = getenv("SSL_CERT_FILE");
  if (configured)
    return *configured ? load_file(store, configured, 0, failure) : 0;
  static const char *paths[] = {
      "/etc/ssl/certs/ca-certificates.crt", "/etc/pki/tls/certs/ca-bundle.crt",
      "/etc/ssl/ca-bundle.pem",
      "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem", "/etc/ssl/cert.pem"};
  for (size_t i = 0; i < sizeof(paths) / sizeof(paths[0]); ++i) {
    int loaded = load_file(store, paths[i], 1, failure);
    if (loaded != 0)
      return loaded;
  }
  return 0;
}

static int system_directories(X509_STORE *store, ScoopTlsStep *failure) {
  const char *configured = getenv("SSL_CERT_DIR");
  int loaded = 0;
  if (!configured) {
    static const char *paths[] = {"/etc/ssl/certs", "/etc/pki/tls/certs"};
    for (size_t i = 0; i < sizeof(paths) / sizeof(paths[0]); ++i) {
      int result = load_directory(store, paths[i], 1, failure);
      if (result < 0)
        return result;
      loaded += result;
    }
    return loaded;
  }
  char *paths = OPENSSL_strdup(configured);
  if (!paths) {
    *failure =
        scoop_tls_work_failure(-5, 0, 0, "CA directory allocation failed");
    return -1;
  }
  char *path = paths;
  while (*path) {
    char *end = strchr(path, ':');
    if (end)
      *end = '\0';
    if (*path) {
      int result = load_directory(store, path, 0, failure);
      if (result < 0) {
        OPENSSL_free(paths);
        return result;
      }
      loaded += result;
    }
    if (!end)
      break;
    path = end + 1;
  }
  OPENSSL_free(paths);
  return loaded;
}

ScoopTlsStep scoop_tls_system_load(ScoopTlsContext *context) {
  X509_STORE *store = SSL_CTX_get_cert_store(context->ssl);
  ScoopTlsStep result = {0};
  int files = system_files(store, &result);
  if (files < 0)
    return result;
  int directories = system_directories(store, &result);
  if (directories < 0)
    return result;
  STACK_OF(X509_OBJECT) *objects = X509_STORE_get0_objects(store);
  int certificates = 0;
  for (int i = 0; i < sk_X509_OBJECT_num(objects); ++i)
    if (X509_OBJECT_get_type(sk_X509_OBJECT_value(objects, i)) == X509_LU_X509)
      certificates = 1;
  if ((files == 0 && directories == 0) || !certificates)
    return scoop_tls_work_failure(-5, 0, 0,
                                  "no system TLS CA source is available");
  return result;
}
