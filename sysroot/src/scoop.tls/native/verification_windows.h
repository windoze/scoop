#ifndef SCOOP_TLS_VERIFICATION_WINDOWS_H
#define SCOOP_TLS_VERIFICATION_WINDOWS_H

#include "system.h"

#include "../../scoop.platform/native/windows_api.h"
#include <wincrypt.h>

ScoopTlsStep scoop_tls_windows_verify(ScoopTlsVerification *work,
                                      HCERTCHAINENGINE engine);

#endif
