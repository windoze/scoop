#ifndef SCOOP_TLS_VERIFICATION_DARWIN_H
#define SCOOP_TLS_VERIFICATION_DARWIN_H
#include "system.h"
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>

SecTrustRef scoop_tls_darwin_trust(ScoopTlsVerification *work,
                                   ScoopTlsStep *failure);
ScoopTlsStep scoop_tls_darwin_evaluate(SecTrustRef trust);
#endif
