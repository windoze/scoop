#include <stdint.h>
#include <stdatomic.h>

static _Atomic int32_t released;

void m24_release_add(int32_t value) { atomic_fetch_add(&released, value); }

int32_t m24_release_count(void) { return atomic_load(&released); }
