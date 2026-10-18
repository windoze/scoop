/* Isolate a real platform trust evaluation without changing machine policy. */
#include "../../native/system.h"
#include <stdlib.h>
#ifdef SCOOP_TARGET_OS_DARWIN
#include "../../native/verification_darwin.h"
#endif

int scoop_test_tls_isolated_verify(ScoopTlsVerification *work,
                                   uint64_t anchor) {
  X509 *root = (X509 *)(uintptr_t)anchor;
  ERR_clear_error();
#ifdef SCOOP_TARGET_OS_DARWIN
  SecTrustRef trust = scoop_tls_darwin_trust(work, &work->result);
  if (trust) {
    unsigned char *der = NULL;
    int count = i2d_X509(root, &der);
    CFDataRef bytes = count > 0 ? CFDataCreate(NULL, der, count) : NULL;
    OPENSSL_free(der);
    SecCertificateRef certificate =
        bytes ? SecCertificateCreateWithData(NULL, bytes) : NULL;
    CFArrayRef anchors = certificate
                             ? CFArrayCreate(NULL, (const void **)&certificate,
                                             1, &kCFTypeArrayCallBacks)
                             : NULL;
    Boolean network = true;
    if (!anchors ||
        SecTrustGetNetworkFetchAllowed(trust, &network) != errSecSuccess ||
        network ||
        SecTrustSetAnchorCertificates(trust, anchors) != errSecSuccess ||
        SecTrustSetAnchorCertificatesOnly(trust, true) != errSecSuccess)
      abort();
    work->result = scoop_tls_darwin_evaluate(trust);
    CFRelease(anchors);
    CFRelease(certificate);
    CFRelease(bytes);
    CFRelease(trust);
  }
#else
  X509_STORE *store = X509_STORE_new();
  if (!store || !X509_STORE_add_cert(store, root))
    abort();
  X509_STORE_free(work->store);
  work->store = store;
  work->result = scoop_tls_verify_platform(work);
#endif
  work->completed = 1;
  return work->result.error == 0;
}
