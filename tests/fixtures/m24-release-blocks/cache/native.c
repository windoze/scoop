#include <stdint.h>
#include <stdatomic.h>

static _Atomic int64_t released;

void m24_release_add64(int64_t value) { atomic_fetch_add(&released, value); }
int64_t m24_release_count64(void) { return atomic_load(&released); }
