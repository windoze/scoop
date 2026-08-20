#include <assert.h>
#include <stdint.h>
#include <stdatomic.h>

static _Atomic int32_t attempts[9];

void m24_release_owner(int32_t id) {
    assert(id > 0 && id < 9);
    assert(atomic_fetch_add(&attempts[id], 1) == 0);
}

int32_t m24_release_owner_count(int32_t id) {
    assert(id > 0 && id < 9);
    return atomic_load(&attempts[id]);
}

int32_t m24_release_invoke(int32_t (*callback)(int32_t, void *),
                           void *context, int32_t value) {
    return callback(value, context);
}
