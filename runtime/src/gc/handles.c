#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "scoop_rt.h"
#include "gc_internal.h"

typedef struct ScoopGcHandleSlot {
    void *object;
    int64_t next_free;
    uint32_t generation;
    bool live;
    bool retired;
} ScoopGcHandleSlot;

static ScoopGcHandleSlot *handles;
static size_t handles_len;
static size_t handles_cap;
static int64_t handles_free = -1;
static void **pinned;
static size_t pinned_len;
static size_t pinned_cap;

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
            scoop_gc_roots_fatal("GcHandle generation exhausted");
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
                scoop_gc_roots_fatal("out of memory growing the handle table");
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
        scoop_gc_roots_fatal("invalid or stale GcHandle");
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
        scoop_gc_roots_fatal("invalid or stale GcHandle");
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
        scoop_gc_roots_fatal("scoop_rt_pin: not a GC heap object");
    }
    scoop_gc_roots_lock();
    bool was_pinned = scoop_gc_update_pin_locked(object, true);
    if (!was_pinned) {
        if (pinned_len == pinned_cap) {
            size_t new_cap = pinned_cap == 0 ? 8 : pinned_cap * 2;
            void **grown = realloc(pinned, new_cap * sizeof *grown);
            if (grown == NULL) {
                scoop_gc_roots_unlock();
                scoop_gc_heap_unlock();
                scoop_gc_roots_fatal("out of memory growing the pinned list");
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
        scoop_gc_roots_fatal("scoop_rt_unpin: not a GC heap object");
    }
    scoop_gc_roots_lock();
    bool was_pinned = scoop_gc_update_pin_locked(object, false);
    if (!was_pinned) {
        scoop_gc_roots_unlock();
        scoop_gc_heap_unlock();
        scoop_gc_roots_fatal("scoop_rt_unpin: object is not pinned");
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
        scoop_gc_roots_fatal("pinned registry is inconsistent");
    }
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    return object;
}

void scoop_gc_visit_handles_locked(ScoopGcRootVisitor visitor) {
    for (size_t index = 0; index < handles_len; index++) {
        if (handles[index].live) {
            visitor.visit_slot(&handles[index].object, visitor.context);
        }
    }
    for (size_t index = 0; index < pinned_len; index++) {
        visitor.visit_slot(&pinned[index], visitor.context);
    }
}
