#include "../../native/system.h"
#include <stdlib.h>

typedef struct {
  ScoopTlsVerification *verification;
  X509 *root;
} IsolatedWork;

int scoop_test_tls_isolated_verify(ScoopTlsVerification *work, uint64_t anchor);

void *scoop_test_tls_isolated_work_new(void *verification, uint64_t anchor) {
  IsolatedWork *work = calloc(1, sizeof(*work));
  if (!work || !X509_up_ref((X509 *)(uintptr_t)anchor))
    abort();
  work->verification = verification;
  work->root = (X509 *)(uintptr_t)anchor;
  return work;
}

static void execute(void *value) {
  IsolatedWork *work = value;
  (void)scoop_test_tls_isolated_verify(work->verification,
                                       (uintptr_t)work->root);
}

static void release(void *value) {
  IsolatedWork *work = value;
  scoop_tls_verification_free(work->verification);
  X509_free(work->root);
  free(work);
}

void *scoop_test_tls_isolated_work_result(IsolatedWork *work) {
  return work->verification;
}

ScoopTlsWorkFunction scoop_test_tls_isolated_work_executor(void) {
  return execute;
}
ScoopTlsWorkFunction scoop_test_tls_isolated_work_releaser(void) {
  return release;
}
