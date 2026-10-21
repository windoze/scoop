#include <errno.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>

/* The actual platform shim calls these two libc wrappers in this fixture. */
static _Atomic int fail_next;
static _Atomic int frees;

void m45_fail_next(void) { atomic_store_explicit(&fail_next, 1, memory_order_relaxed); }
int m45_free_count(void) { return atomic_load_explicit(&frees, memory_order_relaxed); }
int m45_no_memory(void) { return ENOMEM; }

void *m45_realloc(void *pointer, size_t size) {
    if (atomic_exchange_explicit(&fail_next, 0, memory_order_relaxed)) {
        errno = ENOMEM;
        return NULL;
    }
    return realloc(pointer, size);
}

void m45_free(void *pointer) {
    atomic_fetch_add_explicit(&frees, 1, memory_order_relaxed);
    free(pointer);
}
