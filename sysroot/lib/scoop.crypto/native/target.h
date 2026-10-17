#ifndef SCOOP_CRYPTO_TARGET_H
#define SCOOP_CRYPTO_TARGET_H

/* Require the selected package's generated headers before any system search. */
#if defined(SCOOP_TARGET_OS_DARWIN)
#include "../../../native/aarch64-apple-darwin/openssl/include/openssl/opensslconf.h"
#elif defined(SCOOP_TARGET_ENV_MUSL)
#include "../../../native/x86_64-unknown-linux-musl/openssl/include/openssl/opensslconf.h"
#else
#include "../../../native/x86_64-unknown-linux-gnu/openssl/include/openssl/opensslconf.h"
#endif

#endif
