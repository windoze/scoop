#ifndef SCOOP_TEST_NO_CORE_H
#define SCOOP_TEST_NO_CORE_H

#include <assert.h>
#include <sys/resource.h>
#if defined(__linux__)
#include <sys/prctl.h>
#endif

/* A piped Linux core handler may ignore RLIMIT_CORE. Expected-abort tests
 * must not invoke a host crash reporter for every rejected metadata case. */
static inline void scoop_test_disable_core_dumps(void) {
    struct rlimit limit = {0, 0};
    assert(setrlimit(RLIMIT_CORE, &limit) == 0);
#if defined(__linux__)
    assert(prctl(PR_SET_DUMPABLE, 0) == 0);
#endif
}

#endif
