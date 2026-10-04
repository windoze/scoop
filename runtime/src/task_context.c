#include <string.h>

#include "gc/gc_internal.h"
#include "image/registry.h"
#include "managed_entries.h"
#include "task_context.h"
#include "thread.h"
#include "value_shape.h"

enum { CONTEXT_MAX_HEIGHT = 16, CONTEXT_CARD_SHIFT = 9 };

static void context_barrier(const void *object) {
    scoop_gc_card_table[(uintptr_t)object >> CONTEXT_CARD_SHIFT] = 1;
}

static uint32_t context_slot(const uint64_t *cell) {
    uint64_t encoded = *cell;
    if (encoded == 0 || encoded > scoop_image_current()->context_key_count) {
        scoop_shape_fatal("unregistered Context key slot");
    }
    return (uint32_t)(encoded - 1);
}

void *scoop_rt_context_current(void) {
    return scoop_thread_current_required()->current_task_context;
}

void *scoop_rt_context_snapshot(void) {
    ScoopTaskContext *task = scoop_rt_context_current();
    return task == NULL ? NULL : task->root;
}

void *scoop_rt_context_try_get(const uint64_t *cell) {
    uint32_t slot = context_slot(cell);
    unsigned height = scoop_image_current()->context_height;
    ScoopContextNode *node = scoop_rt_context_snapshot();
    for (unsigned level = height; node != NULL && level != 0; level--) {
        void *value = node->slots[(slot >> (2 * (level - 1))) & 3];
        if (level == 1) {
            return value;
        }
        node = value;
    }
    return NULL;
}

void *scoop_rt_context_push_impl(const uint64_t *cell, void *value,
                                 const ScoopTypeDescriptor *node_td,
                                 uintptr_t return_pc, uintptr_t stack_pointer,
                                 uintptr_t frame_pointer) {
    uint32_t slot = context_slot(cell);
    unsigned height = scoop_image_current()->context_height;
    ScoopTaskContext *owner = scoop_rt_context_current();
    if (owner == NULL || value == NULL) {
        scoop_shape_fatal("Context push without a task or binding");
    }
    ScoopContextNode *previous = owner->root;
    ScoopContextNode *path[CONTEXT_MAX_HEIGHT] = {0};
    ScoopContextNode *child = NULL;
    ScoopContextNode *cursor = previous;
    for (unsigned level = height; level != 0; level--) {
        path[level - 1] = cursor;
        if (cursor != NULL && level > 1) {
            cursor = cursor->slots[(slot >> (2 * (level - 1))) & 3];
        }
    }
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    void **slots[CONTEXT_MAX_HEIGHT + 4] = {
        (void **)&owner,
        &value,
        (void **)&previous,
        (void **)&child,
    };
    for (unsigned index = 0; index < CONTEXT_MAX_HEIGHT; index++) {
        slots[index + 4] = (void **)&path[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, CONTEXT_MAX_HEIGHT + 4);
    scoop_thread_poll();
    for (unsigned level = 0; level < height; level++) {
        ScoopContextNode *node = scoop_gc_alloc_internal(node_td, sizeof *node);
        /* Allocation may move every old path node and the child. Reload them
         * from the registered slots before copying or linking. */
        if (path[level] != NULL) {
            memcpy(node->slots, path[level]->slots, sizeof node->slots);
        }
        node->slots[(slot >> (2 * level)) & 3] = level == 0 ? value : child;
        context_barrier(node);
        child = node;
    }
    owner->root = child;
    context_barrier(owner);
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_pop_managed_anchor(&anchor);
    return previous;
}

void scoop_rt_context_restore(void *raw_owner, void *previous) {
    ScoopTaskContext *owner = raw_owner;
    owner->root = previous;
    context_barrier(owner);
}

void *scoop_rt_context_fork_impl(void *root, const ScoopTypeDescriptor *task_td,
                                 uintptr_t return_pc, uintptr_t stack_pointer,
                                 uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    void **slots[] = {&root};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    scoop_thread_poll();
    ScoopTaskContext *task = scoop_gc_alloc_internal(task_td, sizeof *task);
    task->root = root;
    context_barrier(task);
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_pop_managed_anchor(&anchor);
    return task;
}

void *scoop_rt_context_ensure_root_impl(const ScoopTypeDescriptor *task_td,
                                        uintptr_t return_pc, uintptr_t stack_pointer,
                                        uintptr_t frame_pointer) {
    ScoopThreadState *thread = scoop_thread_current_required();
    if (thread->current_task_context == NULL) {
        thread->current_task_context = scoop_rt_context_fork_impl(
            NULL, task_td, return_pc, stack_pointer, frame_pointer);
    }
    return thread->current_task_context;
}

void *scoop_rt_context_enter(void *task) {
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopTaskContext *previous = thread->current_task_context;
    thread->current_task_context = task;
    return previous;
}

void scoop_rt_context_leave(void *previous) {
    scoop_thread_current_required()->current_task_context = previous;
}
