#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "../gc/gc_internal.h"
#include "../image/registry.h"
#include "internal.h"
#include "scoop_rt.h"

static pthread_mutex_t roots_lock = PTHREAD_MUTEX_INITIALIZER;
static void ***roots;
static size_t roots_len;
static size_t roots_cap;

_Noreturn void scoop_gc_roots_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

void scoop_gc_roots_lock(void) {
    if (pthread_mutex_lock(&roots_lock) != 0) {
        scoop_gc_roots_fatal("failed to lock the root registry");
    }
}

void scoop_gc_roots_unlock(void) {
    if (pthread_mutex_unlock(&roots_lock) != 0) {
        scoop_gc_roots_fatal("failed to unlock the root registry");
    }
}

static void root_push(void **slot) {
    if (roots_len == roots_cap) {
        size_t new_cap = roots_cap == 0 ? 16 : roots_cap * 2;
        void ***grown = realloc(roots, new_cap * sizeof *grown);
        if (grown == NULL) {
            scoop_gc_roots_fatal("out of memory growing the root list");
        }
        roots = grown;
        roots_cap = new_cap;
    }
    roots[roots_len++] = slot;
}

bool scoop_gc_is_immortal_object_locked(const void *object) {
    return scoop_image_immortal(scoop_image_current(), object) != NULL;
}

bool scoop_gc_stw_is_stable_object(const void *object) {
    return scoop_image_immortal(scoop_image_current(), object) != NULL;
}

void scoop_rt_gc_add_root(void **slot) {
    if (slot == NULL) {
        scoop_gc_roots_fatal("process root has a null slot address");
    }
    scoop_gc_roots_lock();
    root_push(slot);
    scoop_gc_roots_unlock();
}

void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor) {
    if (visitor.visit_slot == NULL || visitor.visit_region == NULL) {
        scoop_gc_roots_fatal("collector supplied an incomplete root visitor");
    }
    const ScoopImageRegistry *registry = scoop_image_current();
    for (size_t index = 0; index < registry->static_root_count; index++) {
        const ScoopStaticStorageDescriptorV1 *storage = registry->static_roots[index];
        visitor.visit_region(storage->writable_base, storage->scan_program, visitor.context);
    }
    for (size_t index = 0; index < roots_len; index++) {
        visitor.visit_slot(roots[index], visitor.context);
    }
    scoop_gc_visit_handles_locked(visitor);
}

uint64_t scoop_rt_gc_debug_root_count(void) {
    scoop_gc_roots_lock();
    uint64_t count = (uint64_t)roots_len;
    scoop_gc_roots_unlock();
    return count;
}
