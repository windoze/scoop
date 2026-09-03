#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "scoop_rt.h"
#include "gc_internal.h"


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

static const ScoopManagedGlobalDescriptor *image_managed_globals;
static uint64_t image_managed_global_count;
static const ScoopImmortalObjectDescriptor *image_immortal_objects;
static uint64_t image_immortal_object_count;
static bool image_roots_registered;

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

static bool ranges_overlap(uintptr_t left_start, uint64_t left_size,
                           uintptr_t right_start, uint64_t right_size) {
    if (left_size > UINTPTR_MAX - left_start ||
        right_size > UINTPTR_MAX - right_start) {
        scoop_gc_roots_fatal("immortal object range overflows uintptr_t");
    }
    uintptr_t left_end = left_start + (uintptr_t)left_size;
    uintptr_t right_end = right_start + (uintptr_t)right_size;
    return left_start < right_end && right_start < left_end;
}

void scoop_gc_register_image_roots(
    const ScoopManagedGlobalDescriptor *managed_globals,
    uint64_t managed_global_count,
    const ScoopImmortalObjectDescriptor *immortal_objects,
    uint64_t immortal_object_count) {
    scoop_gc_roots_lock();
    if (image_roots_registered) {
        scoop_gc_roots_unlock();
        scoop_gc_roots_fatal("image roots were registered more than once");
    }
    if (managed_globals == NULL || immortal_objects == NULL) {
        scoop_gc_roots_unlock();
        scoop_gc_roots_fatal("image root table symbol is null");
    }
    for (uint64_t index = 0; index < managed_global_count; index++) {
        if (managed_globals[index].writable_base == NULL ||
            managed_globals[index].scan == NULL) {
            scoop_gc_roots_unlock();
            scoop_gc_roots_fatal("managed global descriptor is incomplete");
        }
        for (uint64_t previous = 0; previous < index; previous++) {
            if (managed_globals[index].writable_base ==
                managed_globals[previous].writable_base) {
                scoop_gc_roots_unlock();
                scoop_gc_roots_fatal("managed global storage is registered twice");
            }
        }
    }
    for (uint64_t index = 0; index < immortal_object_count; index++) {
        const ScoopImmortalObjectDescriptor *entry =
            &immortal_objects[index];
        if (entry->object_start == NULL || entry->td == NULL ||
            entry->object_size < sizeof(ScoopObjectHeader) ||
            entry->object_size < entry->td->size) {
            scoop_gc_roots_unlock();
            scoop_gc_roots_fatal("immortal object descriptor is incomplete");
        }
        const ScoopObjectHeader *header = entry->object_start;
        if (header->td != entry->td) {
            scoop_gc_roots_unlock();
            scoop_gc_roots_fatal("immortal object TypeDescriptor does not match header");
        }
        if (entry->td->ref_offsets != NULL) {
            scoop_gc_roots_unlock();
            scoop_gc_roots_fatal("read-only immortal object contains managed references");
        }
        for (uint64_t previous = 0; previous < index; previous++) {
            if (ranges_overlap(
                    (uintptr_t)entry->object_start, entry->object_size,
                    (uintptr_t)immortal_objects[previous].object_start,
                    immortal_objects[previous].object_size)) {
                scoop_gc_roots_unlock();
                scoop_gc_roots_fatal("immortal object ranges overlap");
            }
        }
    }
    image_managed_globals = managed_globals;
    image_managed_global_count = managed_global_count;
    image_immortal_objects = immortal_objects;
    image_immortal_object_count = immortal_object_count;
    image_roots_registered = true;
    scoop_gc_roots_unlock();
}

bool scoop_gc_is_immortal_object_locked(const void *object) {
    for (uint64_t index = 0; index < image_immortal_object_count; index++) {
        if (image_immortal_objects[index].object_start == object) {
            return true;
        }
    }
    return false;
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
    if (!image_roots_registered) {
        scoop_gc_roots_fatal("collector ran before image roots were registered");
    }
    for (uint64_t index = 0; index < image_managed_global_count; index++) {
        visitor.visit_region(image_managed_globals[index].writable_base,
                             image_managed_globals[index].scan,
                             visitor.context);
    }
    for (size_t index = 0; index < roots_len; index++) {
        switch (roots[index].kind) {
        case SCOOP_GC_ROOT_SLOT:
            visitor.visit_slot(roots[index].source.slot, visitor.context);
            break;
        case SCOOP_GC_ROOT_EXTERNAL_OBJECT:
            visitor.visit_external_object(
                roots[index].source.external_object, visitor.context);
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
