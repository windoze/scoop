#include <stdint.h>

_Thread_local int32_t scoop_fixture_tls = 5;
int32_t *scoop_fixture_tls_address(void);
int32_t *scoop_fixture_tls_address(void) { return &scoop_fixture_tls; }
