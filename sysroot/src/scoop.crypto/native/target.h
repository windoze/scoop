#ifndef SCOOP_CRYPTO_TARGET_H
#define SCOOP_CRYPTO_TARGET_H

/* Require the selected package's generated headers before any system search. */
#if defined(SCOOP_TARGET_OS_DARWIN)
#include "../../../lib/aarch64-apple-darwin/openssl/include/openssl/opensslconf.h"
#elif defined(SCOOP_TARGET_ENV_MUSL)
#include "../../../lib/x86_64-unknown-linux-musl/openssl/include/openssl/opensslconf.h"
#elif defined(SCOOP_TARGET_ENV_MSVC_UCRT)
#include "../../../lib/x86_64-pc-windows-msvc-ucrt/openssl/include/openssl/opensslconf.h"
#elif defined(SCOOP_TARGET_ENV_LLVM_MINGW_MSVCRT)
#include "../../../lib/x86_64-w64-windows-llvm-mingw-msvcrt/openssl/include/openssl/opensslconf.h"
#elif defined(SCOOP_TARGET_ENV_LLVM_MINGW_UCRT)
#include "../../../lib/x86_64-w64-windows-llvm-mingw-ucrt/openssl/include/openssl/opensslconf.h"
#else
#include "../../../lib/x86_64-unknown-linux-gnu/openssl/include/openssl/opensslconf.h"
#endif

#endif
