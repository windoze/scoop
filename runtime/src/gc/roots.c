#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "scoop_rt.h"
#include "../thread.h"
#include "gc_internal.h"

#define GC_PIN_BIT UINT64_C(2)

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

typedef struct ScoopGcHandleSlot {
    void *object;
    int64_t next_free;
    uint32_t generation;
    bool live;
    bool retired;
} ScoopGcHandleSlot;

static pthread_mutex_t roots_lock = PTHREAD_MUTEX_INITIALIZER;
static ScoopGcRoot *roots;
static size_t roots_len;
static size_t roots_cap;
static ScoopGcHandleSlot *handles;
static size_t handles_len;
static size_t handles_cap;
static int64_t handles_free = -1;
static void **pinned;
static size_t pinned_len;
static size_t pinned_cap;
static const ScoopManagedGlobalDescriptor *image_managed_globals;
static uint64_t image_managed_global_count;
static const ScoopImmortalObjectDescriptor *image_immortal_objects;
static uint64_t image_immortal_object_count;
static bool image_roots_registered;

static _Noreturn void roots_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

void scoop_gc_roots_lock(void) {
    if (pthread_mutex_lock(&roots_lock) != 0) {
        roots_fatal("failed to lock the root registry");
    }
}

void scoop_gc_roots_unlock(void) {
    if (pthread_mutex_unlock(&roots_lock) != 0) {
        roots_fatal("failed to unlock the root registry");
    }
}

static void root_push(ScoopGcRoot root) {
    if (roots_len == roots_cap) {
        size_t new_cap = roots_cap == 0 ? 16 : roots_cap * 2;
        ScoopGcRoot *grown = realloc(roots, new_cap * sizeof *grown);
        if (grown == NULL) {
            roots_fatal("out of memory growing the root list");
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
        roots_fatal("immortal object range overflows uintptr_t");
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
        roots_fatal("image roots were registered more than once");
    }
    if (managed_globals == NULL || immortal_objects == NULL) {
        scoop_gc_roots_unlock();
        roots_fatal("image root table symbol is null");
    }
    for (uint64_t index = 0; index < managed_global_count; index++) {
        if (managed_globals[index].writable_base == NULL ||
            managed_globals[index].scan == NULL) {
            scoop_gc_roots_unlock();
            roots_fatal("managed global descriptor is incomplete");
        }
        for (uint64_t previous = 0; previous < index; previous++) {
            if (managed_globals[index].writable_base ==
                managed_globals[previous].writable_base) {
                scoop_gc_roots_unlock();
                roots_fatal("managed global storage is registered twice");
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
            roots_fatal("immortal object descriptor is incomplete");
        }
        const ScoopObjectHeader *header = entry->object_start;
        if (header->td != entry->td) {
            scoop_gc_roots_unlock();
            roots_fatal("immortal object TypeDescriptor does not match header");
        }
        if (entry->td->ref_offsets != NULL) {
            scoop_gc_roots_unlock();
            roots_fatal("read-only immortal object contains managed references");
        }
        for (uint64_t previous = 0; previous < index; previous++) {
            if (ranges_overlap(
                    (uintptr_t)entry->object_start, entry->object_size,
                    (uintptr_t)immortal_objects[previous].object_start,
                    immortal_objects[previous].object_size)) {
                scoop_gc_roots_unlock();
                roots_fatal("immortal object ranges overlap");
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

void scoop_rt_gc_add_root(void **slot) {
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
    roots_fatal("attempted to remove an unknown external object root");
}

void scoop_rt_push_native_roots(ScoopNativeRootFrame *frame, void ***slots,
                                uint64_t count) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode =
        atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        roots_fatal(
            "native roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || (count != 0 && slots == NULL)) {
        roots_fatal("invalid native root frame");
    }
    for (ScoopNativeRootFrame *active = thread->native_roots; active != NULL;
         active = active->previous) {
        if (active == frame) {
            roots_fatal("native root frame is already active");
        }
    }
    for (uint64_t index = 0; index < count; index++) {
        if (slots[index] == NULL) {
            roots_fatal("native root frame contains a null slot address");
        }
    }
    frame->previous = thread->native_roots;
    frame->slots = slots;
    frame->count = count;
    thread->native_roots = frame;
}

void scoop_rt_pop_native_roots(ScoopNativeRootFrame *frame) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopThreadMode mode =
        atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode != SCOOP_THREAD_MANAGED && mode != SCOOP_THREAD_NATIVE_BORROWED) {
        roots_fatal(
            "native roots may only change in managed or native-borrowed mode");
    }
    if (frame == NULL || thread->native_roots != frame) {
        roots_fatal("native root frames must be popped in LIFO order");
    }
    thread->native_roots = frame->previous;
    frame->previous = NULL;
    frame->slots = NULL;
    frame->count = 0;
}

static uint64_t handle_encode(size_t index, uint32_t generation) {
    return ((uint64_t)generation << 32) | ((uint64_t)index + 1);
}

static ScoopGcHandleSlot *handle_resolve_locked(uint64_t handle) {
    uint32_t encoded_slot = (uint32_t)handle;
    uint32_t generation = (uint32_t)(handle >> 32);
    if (encoded_slot == 0 || generation == 0) {
        return NULL;
    }
    size_t index = (size_t)encoded_slot - 1;
    if (index >= handles_len) {
        return NULL;
    }
    ScoopGcHandleSlot *slot = &handles[index];
    if (!slot->live || slot->generation != generation) {
        return NULL;
    }
    return slot;
}

uint64_t scoop_rt_get_handle(const void *object) {
    if (object == NULL) {
        return 0;
    }
    scoop_gc_roots_lock();
    size_t index;
    if (handles_free >= 0) {
        index = (size_t)handles_free;
        ScoopGcHandleSlot *slot = &handles[index];
        handles_free = slot->next_free;
        slot->generation++;
        if (slot->generation == 0 || slot->retired) {
            scoop_gc_roots_unlock();
            roots_fatal("GcHandle generation exhausted");
        }
        slot->object = (void *)object;
        slot->next_free = -1;
        slot->live = true;
    } else {
        if (handles_len == handles_cap) {
            size_t new_cap = handles_cap == 0 ? 16 : handles_cap * 2;
            ScoopGcHandleSlot *grown =
                realloc(handles, new_cap * sizeof *grown);
            if (grown == NULL) {
                scoop_gc_roots_unlock();
                roots_fatal("out of memory growing the handle table");
            }
            handles = grown;
            handles_cap = new_cap;
        }
        index = handles_len++;
        handles[index] = (ScoopGcHandleSlot){
            .object = (void *)object,
            .next_free = -1,
            .generation = 1,
            .live = true,
            .retired = false,
        };
    }
    uint64_t handle = handle_encode(index, handles[index].generation);
    scoop_gc_roots_unlock();
    return handle;
}

const void *scoop_rt_release_handle(uint64_t handle) {
    if (handle == 0) {
        return NULL;
    }
    scoop_gc_roots_lock();
    ScoopGcHandleSlot *slot = handle_resolve_locked(handle);
    if (slot == NULL) {
        scoop_gc_roots_unlock();
        roots_fatal("invalid or stale GcHandle");
    }
    const void *object = slot->object;
    slot->object = NULL;
    slot->live = false;
    size_t index = (size_t)((uint32_t)handle - 1);
    if (slot->generation == UINT32_MAX) {
        slot->retired = true;
        slot->next_free = -1;
    } else {
        slot->next_free = handles_free;
        handles_free = (int64_t)index;
    }
    scoop_gc_roots_unlock();
    return object;
}

const void *scoop_rt_resolve_handle(uint64_t handle) {
    if (handle == 0) {
        return NULL;
    }
    scoop_gc_roots_lock();
    ScoopGcHandleSlot *slot = handle_resolve_locked(handle);
    if (slot == NULL) {
        scoop_gc_roots_unlock();
        roots_fatal("invalid or stale GcHandle");
    }
    const void *object = slot->object;
    scoop_gc_roots_unlock();
    return object;
}

const void *scoop_rt_pin(const void *object) {
    if (object == NULL) {
        return NULL;
    }
    scoop_gc_heap_lock();
    if (!scoop_gc_is_object_start_locked(object)) {
        scoop_gc_heap_unlock();
        roots_fatal("scoop_rt_pin: not a GC heap object");
    }
    scoop_gc_roots_lock();
    ScoopObjectHeader *header = (ScoopObjectHeader *)object;
    uint64_t old_word = __atomic_fetch_or(&header->gc_word, GC_PIN_BIT,
                                          __ATOMIC_ACQ_REL);
    if ((old_word & GC_PIN_BIT) == 0) {
        if (pinned_len == pinned_cap) {
            size_t new_cap = pinned_cap == 0 ? 8 : pinned_cap * 2;
            void **grown = realloc(pinned, new_cap * sizeof *grown);
            if (grown == NULL) {
                scoop_gc_roots_unlock();
                scoop_gc_heap_unlock();
                roots_fatal("out of memory growing the pinned list");
            }
            pinned = grown;
            pinned_cap = new_cap;
        }
        pinned[pinned_len++] = (void *)object;
    }
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    return object;
}

const void *scoop_rt_unpin(const void *object) {
    if (object == NULL) {
        return NULL;
    }
    scoop_gc_heap_lock();
    if (!scoop_gc_is_object_start_locked(object)) {
        scoop_gc_heap_unlock();
        roots_fatal("scoop_rt_unpin: not a GC heap object");
    }
    scoop_gc_roots_lock();
    ScoopObjectHeader *header = (ScoopObjectHeader *)object;
    uint64_t old_word = __atomic_fetch_and(&header->gc_word, ~GC_PIN_BIT,
                                           __ATOMIC_ACQ_REL);
    if ((old_word & GC_PIN_BIT) == 0) {
        scoop_gc_roots_unlock();
        scoop_gc_heap_unlock();
        roots_fatal("scoop_rt_unpin: object is not pinned");
    }
    bool found = false;
    for (size_t index = 0; index < pinned_len; index++) {
        if (pinned[index] == object) {
            pinned[index] = pinned[--pinned_len];
            found = true;
            break;
        }
    }
    if (!found) {
        scoop_gc_roots_unlock();
        scoop_gc_heap_unlock();
        roots_fatal("pinned registry is inconsistent");
    }
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    return object;
}

void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor) {
    if (visitor.visit_slot == NULL || visitor.visit_external_object == NULL ||
        visitor.visit_region == NULL) {
        roots_fatal("collector supplied an incomplete root visitor");
    }
    if (!image_roots_registered) {
        roots_fatal("collector ran before image roots were registered");
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
    for (size_t index = 0; index < handles_len; index++) {
        if (handles[index].live) {
            visitor.visit_slot(&handles[index].object, visitor.context);
        }
    }
    for (size_t index = 0; index < pinned_len; index++) {
        visitor.visit_slot(&pinned[index], visitor.context);
    }
}

uint64_t scoop_rt_gc_debug_root_count(void) {
    scoop_gc_roots_lock();
    uint64_t count = (uint64_t)roots_len;
    scoop_gc_roots_unlock();
    return count;
}

uint64_t scoop_rt_gc_debug_native_root_count(void) {
    ScoopThreadState *thread = scoop_thread_current();
    if (thread == NULL) {
        return 0;
    }
    uint64_t count = 0;
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        count += frame->count;
    }
    return count;
}
