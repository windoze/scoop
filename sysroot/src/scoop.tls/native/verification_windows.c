#include "verification_windows.h"

ScoopTlsStep scoop_tls_system_load(ScoopTlsContext *context) {
  (void)context;
  // Crypt32 applies the current user and machine trust policy to each peer.
  return (ScoopTlsStep){0};
}

static ScoopTlsStep trust_failure(DWORD code) {
  WCHAR description[512];
  char text[1536] = "Windows system TLS trust evaluation failed";
  if (FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                     NULL, code, 0, description,
                     sizeof(description) / sizeof(description[0]), NULL))
    (void)WideCharToMultiByte(CP_UTF8, 0, description, -1, text, sizeof(text),
                              NULL, NULL);
  ERR_clear_error();
  return scoop_tls_work_failure(-3, 0, (int64_t)code, text);
}

static PCCERT_CONTEXT certificate(X509 *value) {
  unsigned char *der = NULL;
  int count = i2d_X509(value, &der);
  if (count <= 0) {
    SetLastError(ERROR_INVALID_DATA);
    return NULL;
  }
  PCCERT_CONTEXT result =
      CertCreateCertificateContext(X509_ASN_ENCODING, der, (DWORD)count);
  DWORD error = result ? ERROR_SUCCESS : GetLastError();
  OPENSSL_free(der);
  if (!result)
    SetLastError(error);
  return result;
}

static WCHAR *peer_name(ScoopTlsVerification *work) {
  const char *dns = X509_VERIFY_PARAM_get0_host(work->parameters, 0);
  char *ip = dns ? NULL : X509_VERIFY_PARAM_get1_ip_asc(work->parameters);
  const char *name = dns ? dns : ip;
  WCHAR *result = NULL;
  DWORD error = ERROR_INVALID_NAME;
  if (name) {
    int size =
        MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, name, -1, NULL, 0);
    if (size > 0) {
      result = OPENSSL_malloc((size_t)size * sizeof(*result));
      if (!result)
        error = ERROR_NOT_ENOUGH_MEMORY;
      else if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, name, -1,
                                    result, size)) {
        error = GetLastError();
        OPENSSL_free(result);
        result = NULL;
      }
    } else
      error = GetLastError();
  }
  OPENSSL_free(ip);
  if (!result)
    SetLastError(error);
  return result;
}

ScoopTlsStep scoop_tls_windows_verify(ScoopTlsVerification *work,
                                      HCERTCHAINENGINE engine) {
  ScoopTlsStep result = {0};
  PCCERT_CONTEXT leaf = NULL;
  PCCERT_CHAIN_CONTEXT chain = NULL;
  WCHAR *name = NULL;
  HCERTSTORE peers = CertOpenStore(CERT_STORE_PROV_MEMORY, 0, 0,
                                   CERT_STORE_CREATE_NEW_FLAG, NULL);
  if (!peers)
    goto failed;
  leaf = certificate(work->leaf);
  if (!leaf)
    goto failed;
  for (int i = 0; i < sk_X509_num(work->chain); ++i) {
    PCCERT_CONTEXT item = certificate(sk_X509_value(work->chain, i));
    if (!item)
      goto failed;
    BOOL added = CertAddCertificateContextToStore(peers, item,
                                                  CERT_STORE_ADD_ALWAYS, NULL);
    DWORD error = added ? ERROR_SUCCESS : GetLastError();
    CertFreeCertificateContext(item);
    if (!added) {
      SetLastError(error);
      goto failed;
    }
  }
  name = peer_name(work);
  if (!name)
    goto failed;
  char *usage = szOID_PKIX_KP_SERVER_AUTH;
  CERT_CHAIN_PARA parameters = {0};
  parameters.cbSize = sizeof(parameters);
  parameters.RequestedUsage.dwType = USAGE_MATCH_TYPE_AND;
  parameters.RequestedUsage.Usage.cUsageIdentifier = 1;
  parameters.RequestedUsage.Usage.rgpszUsageIdentifier = &usage;
  DWORD flags = CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL | CERT_CHAIN_DISABLE_AIA |
                CERT_CHAIN_DISABLE_AUTH_ROOT_AUTO_UPDATE |
                CERT_CHAIN_REVOCATION_CHECK_CACHE_ONLY;
  if (!CertGetCertificateChain(engine, leaf, NULL, peers, &parameters, flags,
                               NULL, &chain))
    goto failed;
  SSL_EXTRA_CERT_CHAIN_POLICY_PARA ssl = {0};
  ssl.cbSize = sizeof(ssl);
  ssl.dwAuthType = AUTHTYPE_SERVER;
  ssl.pwszServerName = name;
  CERT_CHAIN_POLICY_PARA policy = {0};
  policy.cbSize = sizeof(policy);
  policy.pvExtraPolicyPara = &ssl;
  CERT_CHAIN_POLICY_STATUS status = {0};
  status.cbSize = sizeof(status);
  if (!CertVerifyCertificateChainPolicy(CERT_CHAIN_POLICY_SSL, chain, &policy,
                                        &status))
    goto failed;
  if (status.dwError)
    result = trust_failure(status.dwError);
  goto done;
failed:
  result = trust_failure(GetLastError());
done:
  if (chain)
    CertFreeCertificateChain(chain);
  if (leaf)
    CertFreeCertificateContext(leaf);
  if (peers)
    CertCloseStore(peers, 0);
  OPENSSL_free(name);
  return result;
}

ScoopTlsStep scoop_tls_verify_platform(ScoopTlsVerification *work) {
  return scoop_tls_windows_verify(work, HCCE_CURRENT_USER);
}
