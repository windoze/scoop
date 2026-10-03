#include <assert.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>

static _Atomic int32_t released;

void *m24_create(int32_t value) {
    int32_t *resource = malloc(sizeof(*resource));
    assert(resource != NULL);
    *resource = value;
    return resource;
}

void m24_drop(void *resource) {
    assert(resource != NULL);
    atomic_fetch_add(&released, *(int32_t *)resource);
    free(resource);
}

int32_t m24_count(void) { return atomic_load(&released); }
