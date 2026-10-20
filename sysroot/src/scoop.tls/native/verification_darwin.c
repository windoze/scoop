#include "verification_darwin.h"

ScoopTlsStep scoop_tls_system_load(ScoopTlsContext *context) {
  (void)context;
  // SecTrust resolves the current system/user/admin policy for each peer.
  return (ScoopTlsStep){0};
}

static ScoopTlsStep trust_failure(int64_t code, CFStringRef description) {
  char text[256] = "macOS system TLS trust evaluation failed";
  if (description)
    (void)CFStringGetCString(description, text, sizeof(text),
                             kCFStringEncodingUTF8);
  ERR_clear_error();
  return scoop_tls_work_failure(-3, 0, code, text);
}

static ScoopTlsStep status_failure(OSStatus status) {
  CFStringRef description = SecCopyErrorMessageString(status, NULL);
  ScoopTlsStep result = trust_failure(status, description);
  if (description)
    CFRelease(description);
  return result;
}

static SecCertificateRef certificate(X509 *value) {
  unsigned char *der = NULL;
  int count = i2d_X509(value, &der);
  if (count <= 0)
    return NULL;
  CFDataRef bytes = CFDataCreate(NULL, der, count);
  OPENSSL_free(der);
  if (!bytes)
    return NULL;
  SecCertificateRef result = SecCertificateCreateWithData(NULL, bytes);
  CFRelease(bytes);
  return result;
}

SecTrustRef scoop_tls_darwin_trust(ScoopTlsVerification *work,
                                   ScoopTlsStep *failure) {
  CFMutableArrayRef chain =
      CFArrayCreateMutable(NULL, 0, &kCFTypeArrayCallBacks);
  CFStringRef host = NULL;
  SecPolicyRef policy = NULL;
  SecTrustRef trust = NULL;
  if (!chain)
    goto allocation;
  for (int i = -1; i < sk_X509_num(work->chain); ++i) {
    X509 *value = i < 0 ? work->leaf : sk_X509_value(work->chain, i);
    if (i >= 0 && X509_cmp(value, work->leaf) == 0)
      continue;
    SecCertificateRef item = certificate(value);
    if (!item)
      goto allocation;
    CFArrayAppendValue(chain, item);
    CFRelease(item);
  }
  const char *dns = X509_VERIFY_PARAM_get0_host(work->parameters, 0);
  char *ip = dns ? NULL : X509_VERIFY_PARAM_get1_ip_asc(work->parameters);
  const char *name = dns ? dns : ip;
  if (name)
    host = CFStringCreateWithCString(NULL, name, kCFStringEncodingUTF8);
  OPENSSL_free(ip);
  if (!host)
    goto allocation;
  policy = SecPolicyCreateSSL(true, host);
  if (!policy)
    goto allocation;
  OSStatus status = SecTrustCreateWithCertificates(chain, policy, &trust);
  if (status == errSecSuccess)
    status = SecTrustSetNetworkFetchAllowed(trust, false);
  if (status != errSecSuccess) {
    *failure = status_failure(status);
    if (trust)
      CFRelease(trust);
    trust = NULL;
  }
  goto done;
allocation:
  *failure = scoop_tls_work_failure(-3, 0, errSecAllocate,
                                    "system TLS policy allocation failed");
done:
  if (policy)
    CFRelease(policy);
  if (host)
    CFRelease(host);
  if (chain)
    CFRelease(chain);
  return trust;
}

ScoopTlsStep scoop_tls_darwin_evaluate(SecTrustRef trust) {
  CFErrorRef error = NULL;
  Boolean valid = SecTrustEvaluateWithError(trust, &error);
  ScoopTlsStep result = {0};
  if (!valid) {
    CFStringRef description = error ? CFErrorCopyDescription(error) : NULL;
    result = trust_failure(error ? CFErrorGetCode(error) : errSecNotTrusted,
                           description);
    if (description)
      CFRelease(description);
  }
  if (error)
    CFRelease(error);
  return result;
}

ScoopTlsStep scoop_tls_verify_platform(ScoopTlsVerification *work) {
  ScoopTlsStep result = {0};
  SecTrustRef trust = scoop_tls_darwin_trust(work, &result);
  if (trust) {
    result = scoop_tls_darwin_evaluate(trust);
    CFRelease(trust);
  }
  return result;
}
