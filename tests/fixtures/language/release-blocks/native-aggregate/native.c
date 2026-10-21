#include <stdint.h>
#include <stdatomic.h>

typedef struct {
    int64_t first;
    int64_t second;
    int64_t third;
} Wide;

static _Atomic int64_t released;

Wide m24_release_rotate(Wide value) {
    return (Wide){value.second, value.third, value.first};
}

void m24_release_wide(Wide value) {
    atomic_fetch_add(&released, value.first + value.second + value.third);
}

int64_t m24_release_wide_count(void) { return atomic_load(&released); }
