#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "../image/registry.h"
#include "gc_internal.h"
#include "scoop_rt.h"

typedef enum ScoopGcRootKind {
    SCOOP_GC_ROOT_SLOT,
    SCOOP_GC_ROOT_EXTERNAL_OBJECT,
} ScoopGcRootKind;

typedef struct ScoopGcRoot {
    ScoopGcRootKind kind;
    union {
        void **slot;
        const void *external_object;
    } source;
} ScoopGcRoot;

static pthread_mutex_t roots_lock = PTHREAD_MUTEX_INITIALIZER;
static ScoopGcRoot *roots;
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

static void root_push(ScoopGcRoot root) {
    if (roots_len == roots_cap) {
        size_t new_cap = roots_cap == 0 ? 16 : roots_cap * 2;
        ScoopGcRoot *grown = realloc(roots, new_cap * sizeof *grown);
        if (grown == NULL) {
            scoop_gc_roots_fatal("out of memory growing the root list");
        }
        roots = grown;
        roots_cap = new_cap;
    }
    roots[roots_len++] = root;
}

bool scoop_gc_is_immortal_object_locked(const void *object) {
    return scoop_image_immortal(scoop_image_current(), object) != NULL;
}

bool scoop_gc_is_external_object_locked(const void *object) {
    for (size_t index = 0; index < roots_len; index++) {
        if (roots[index].kind == SCOOP_GC_ROOT_EXTERNAL_OBJECT &&
            roots[index].source.external_object == object) {
            return true;
        }
    }
    return false;
}

bool scoop_gc_is_published_object(const void *object) {
    if (object == NULL) {
        return false;
    }

    /* Collection takes these locks in heap -> roots order while classifying
     * stable targets.  Keep the same order here so a NoGC runtime entry can
     * validate a throw source without racing root-table mutation. */
    scoop_gc_heap_lock();
    bool published = scoop_gc_is_object_start_locked(object);
    scoop_gc_roots_lock();
    published = published || scoop_gc_is_immortal_object_locked(object) ||
                scoop_gc_is_external_object_locked(object);
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    return published;
}

void scoop_rt_gc_add_root(void **slot) {
    if (slot == NULL) {
        scoop_gc_roots_fatal("process root has a null slot address");
    }
    scoop_gc_roots_lock();
    root_push((ScoopGcRoot){
        .kind = SCOOP_GC_ROOT_SLOT,
        .source.slot = slot,
    });
    scoop_gc_roots_unlock();
}

void scoop_rt_gc_add_root_object(const void *object) {
    scoop_gc_roots_lock();
    root_push((ScoopGcRoot){
        .kind = SCOOP_GC_ROOT_EXTERNAL_OBJECT,
        .source.external_object = object,
    });
    scoop_gc_roots_unlock();
}

void scoop_rt_gc_remove_root_object(const void *object) {
    scoop_gc_roots_lock();
    for (size_t index = 0; index < roots_len; index++) {
        if (roots[index].kind == SCOOP_GC_ROOT_EXTERNAL_OBJECT &&
            roots[index].source.external_object == object) {
            roots[index] = roots[--roots_len];
            scoop_gc_roots_unlock();
            return;
        }
    }
    scoop_gc_roots_unlock();
    scoop_gc_roots_fatal("attempted to remove an unknown external object root");
}

void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor) {
    if (visitor.visit_slot == NULL || visitor.visit_external_object == NULL ||
        visitor.visit_region == NULL) {
        scoop_gc_roots_fatal("collector supplied an incomplete root visitor");
    }
    const ScoopImageRegistry *registry = scoop_image_current();
    for (size_t index = 0; index < registry->static_root_count; index++) {
        const ScoopStaticStorageDescriptorV1 *storage = registry->static_roots[index];
        visitor.visit_region(storage->writable_base, storage->scan_program,
                             visitor.context);
    }
    for (size_t index = 0; index < roots_len; index++) {
        switch (roots[index].kind) {
        case SCOOP_GC_ROOT_SLOT:
            visitor.visit_slot(roots[index].source.slot, visitor.context);
            break;
        case SCOOP_GC_ROOT_EXTERNAL_OBJECT:
            visitor.visit_external_object(roots[index].source.external_object,
                                          visitor.context);
            break;
        }
    }
    scoop_gc_visit_handles_locked(visitor);
}

uint64_t scoop_rt_gc_debug_root_count(void) {
    scoop_gc_roots_lock();
    uint64_t count = (uint64_t)roots_len;
    scoop_gc_roots_unlock();
    return count;
}
