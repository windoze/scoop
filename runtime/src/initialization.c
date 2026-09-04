#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gc/gc_internal.h"
#include "managed_entries.h"
#include "thread/internal.h"

enum {
    SCOOP_INIT_UNINITIALIZED = 0,
    SCOOP_INIT_INITIALIZING = 1,
    SCOOP_INIT_INITIALIZED = 2,
    SCOOP_INIT_FAILED = 3,
};

enum {
    SCOOP_INIT_RUN_INITIALIZER = 0,
    SCOOP_INIT_READY = 1,
    SCOOP_INIT_RESULT_FAILED = 2,
    SCOOP_INIT_CYCLE = 3,
};

extern const ScoopTypeDescriptor scoop_td_String;

static _Noreturn void initialization_fatal(const char *message) {
    fprintf(stderr, "scoop initialization: %s\n", message);
    abort();
}

static void require_unit(const ScoopInitializationUnitDescriptor *unit) {
    if (unit == NULL || unit->stable_key == NULL || unit->stable_key[0] == '\0' ||
        unit->cell == NULL || unit->storage == NULL || unit->failure_root == NULL ||
        unit->initializer_entry == NULL || unit->ensure_entry == NULL) {
        initialization_fatal("initialization unit descriptor is incomplete");
    }
}

static void push_unit(ScoopThreadState *thread,
                      const ScoopInitializationUnitDescriptor *unit) {
    if (thread->initialization_stack_len == thread->initialization_stack_cap) {
        size_t capacity = thread->initialization_stack_cap == 0
                              ? 8
                              : thread->initialization_stack_cap * 2;
        const ScoopInitializationUnitDescriptor **grown =
            realloc(thread->initialization_stack, capacity * sizeof *grown);
        if (grown == NULL) {
            initialization_fatal("out of memory growing initialization dependency stack");
        }
        thread->initialization_stack = grown;
        thread->initialization_stack_cap = capacity;
    }
    thread->initialization_stack[thread->initialization_stack_len++] = unit;
}

static void pop_unit(ScoopThreadState *thread,
                     const ScoopInitializationUnitDescriptor *unit) {
    if (thread->initialization_stack_len == 0 ||
        thread->initialization_stack[thread->initialization_stack_len - 1] != unit) {
        initialization_fatal("initialization dependency stack is corrupt");
    }
    thread->initialization_stack_len--;
}

static void append_path(char **buffer, size_t *length, size_t *capacity,
                        const char *text) {
    size_t text_len = strlen(text);
    if (*length > SIZE_MAX - text_len - 1) {
        initialization_fatal("initialization cycle path is too large");
    }
    size_t needed = *length + text_len + 1;
    if (needed > *capacity) {
        size_t grown_capacity = *capacity == 0 ? 128 : *capacity;
        while (grown_capacity < needed) {
            if (grown_capacity > SIZE_MAX / 2) {
                initialization_fatal("initialization cycle path is too large");
            }
            grown_capacity *= 2;
        }
        char *grown = realloc(*buffer, grown_capacity);
        if (grown == NULL) {
            initialization_fatal("out of memory building initialization cycle path");
        }
        *buffer = grown;
        *capacity = grown_capacity;
    }
    memcpy(*buffer + *length, text, text_len + 1);
    *length += text_len;
}

static void set_cycle_path(
    ScoopThreadState *thread,
    const ScoopInitializationUnitDescriptor *const *units,
    size_t unit_count) {
    char *path = NULL;
    size_t length = 0;
    size_t capacity = 0;
    append_path(&path, &length, &capacity, "initialization cycle: ");
    for (size_t index = 0; index < unit_count; index++) {
        if (index != 0) {
            append_path(&path, &length, &capacity, " -> ");
        }
        append_path(&path, &length, &capacity, units[index]->stable_key);
    }
    free(thread->initialization_cycle_path);
    thread->initialization_cycle_path = path;
}

static void same_thread_cycle(ScoopThreadState *thread,
                              const ScoopInitializationUnitDescriptor *unit) {
    size_t start = thread->initialization_stack_len;
    for (size_t index = 0; index < thread->initialization_stack_len; index++) {
        if (thread->initialization_stack[index] == unit) {
            start = index;
            break;
        }
    }
    if (start == thread->initialization_stack_len) {
        initialization_fatal("initializing owner does not contain its unit on the stack");
    }
    size_t count = thread->initialization_stack_len - start + 1;
    const ScoopInitializationUnitDescriptor **path = malloc(count * sizeof *path);
    if (path == NULL) {
        initialization_fatal("out of memory building initialization cycle");
    }
    for (size_t index = start; index < thread->initialization_stack_len; index++) {
        path[index - start] = thread->initialization_stack[index];
    }
    path[count - 1] = unit;
    set_cycle_path(thread, path, count);
    free(path);
}

static bool cross_thread_cycle(ScoopThreadState *thread,
                               const ScoopInitializationUnitDescriptor *unit) {
    size_t capacity = 8;
    size_t count = 0;
    const ScoopInitializationUnitDescriptor **path = malloc(capacity * sizeof *path);
    if (path == NULL) {
        initialization_fatal("out of memory building initialization wait path");
    }
    if (thread->initialization_stack_len != 0) {
        path[count++] =
            thread->initialization_stack[thread->initialization_stack_len - 1];
    }
    path[count++] = unit;
    ScoopThreadState *owner = unit->cell->owner_thread;
    while (owner != NULL && owner != thread) {
        const ScoopInitializationUnitDescriptor *wait = owner->initialization_wait;
        if (wait == NULL || wait->cell->state != SCOOP_INIT_INITIALIZING) {
            free(path);
            return false;
        }
        if (count == capacity) {
            capacity *= 2;
            const ScoopInitializationUnitDescriptor **grown =
                realloc(path, capacity * sizeof *grown);
            if (grown == NULL) {
                free(path);
                initialization_fatal("out of memory growing initialization wait path");
            }
            path = grown;
        }
        path[count++] = wait;
        owner = wait->cell->owner_thread;
    }
    if (owner != thread) {
        free(path);
        return false;
    }
    if (thread->initialization_stack_len != 0 &&
        path[count - 1] != thread->initialization_stack[thread->initialization_stack_len - 1]) {
        if (count == capacity) {
            capacity++;
            const ScoopInitializationUnitDescriptor **grown =
                realloc(path, capacity * sizeof *grown);
            if (grown == NULL) {
                free(path);
                initialization_fatal("out of memory closing initialization wait path");
            }
            path = grown;
        }
        path[count++] =
            thread->initialization_stack[thread->initialization_stack_len - 1];
    }
    set_cycle_path(thread, path, count);
    free(path);
    return true;
}

static void wait_for_unit(ScoopThreadState *thread,
                          const ScoopInitializationUnitDescriptor *unit) {
    thread->initialization_wait = unit;
    thread->parked_from = SCOOP_THREAD_MANAGED;
    atomic_store_explicit(&thread->mode, SCOOP_THREAD_PARKED, memory_order_release);
    for (;;) {
        uint64_t epoch =
            atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire);
        atomic_store_explicit(&thread->observed_gc_epoch, epoch, memory_order_release);
        scoop_thread_world_broadcast();
        if (unit->cell->state != SCOOP_INIT_INITIALIZING &&
            atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) ==
                SCOOP_WORLD_RUNNING) {
            break;
        }
        scoop_thread_world_wait();
    }
    thread->initialization_wait = NULL;
    atomic_store_explicit(&thread->mode, SCOOP_THREAD_MANAGED, memory_order_release);
}

static uint64_t init_enter(const ScoopInitializationUnitDescriptor *unit) {
    require_unit(unit);
    ScoopThreadState *thread = scoop_thread_current_required();
    scoop_thread_require_managed();
    scoop_thread_registry_lock();
    for (;;) {
        switch (unit->cell->state) {
        case SCOOP_INIT_UNINITIALIZED:
            unit->cell->state = SCOOP_INIT_INITIALIZING;
            unit->cell->owner_thread = thread;
            push_unit(thread, unit);
            scoop_thread_registry_unlock();
            return SCOOP_INIT_RUN_INITIALIZER;
        case SCOOP_INIT_INITIALIZED:
            scoop_thread_registry_unlock();
            return SCOOP_INIT_READY;
        case SCOOP_INIT_FAILED:
            if (*unit->failure_root == NULL) {
                scoop_thread_registry_unlock();
                initialization_fatal("failed initialization unit has no exception root");
            }
            scoop_thread_registry_unlock();
            return SCOOP_INIT_RESULT_FAILED;
        case SCOOP_INIT_INITIALIZING:
            if (unit->cell->owner_thread == thread) {
                same_thread_cycle(thread, unit);
                scoop_thread_registry_unlock();
                return SCOOP_INIT_CYCLE;
            }
            thread->initialization_wait = unit;
            if (cross_thread_cycle(thread, unit)) {
                thread->initialization_wait = NULL;
                scoop_thread_registry_unlock();
                return SCOOP_INIT_CYCLE;
            }
            thread->initialization_wait = NULL;
            wait_for_unit(thread, unit);
            break;
        default:
            scoop_thread_registry_unlock();
            initialization_fatal("initialization cell has an invalid state");
        }
    }
}

static void init_succeed(const ScoopInitializationUnitDescriptor *unit) {
    require_unit(unit);
    ScoopThreadState *thread = scoop_thread_current_required();
    scoop_thread_registry_lock();
    if (unit->cell->state != SCOOP_INIT_INITIALIZING ||
        unit->cell->owner_thread != thread || *unit->failure_root != NULL) {
        scoop_thread_registry_unlock();
        initialization_fatal("invalid initialization success publication");
    }
    pop_unit(thread, unit);
    unit->cell->owner_thread = NULL;
    unit->cell->state = SCOOP_INIT_INITIALIZED;
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
}

static void init_fail(const ScoopInitializationUnitDescriptor *unit, void *exception) {
    require_unit(unit);
    if (exception == NULL) {
        initialization_fatal("initialization failure publication has a null exception");
    }
    ScoopThreadState *thread = scoop_thread_current_required();
    scoop_thread_registry_lock();
    if (unit->cell->state != SCOOP_INIT_INITIALIZING ||
        unit->cell->owner_thread != thread || *unit->failure_root != NULL) {
        scoop_thread_registry_unlock();
        initialization_fatal("invalid initialization failure publication");
    }
    *unit->failure_root = exception;
    pop_unit(thread, unit);
    unit->cell->owner_thread = NULL;
    unit->cell->state = SCOOP_INIT_FAILED;
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
}

static void *init_failure(const ScoopInitializationUnitDescriptor *unit) {
    require_unit(unit);
    scoop_thread_registry_lock();
    if (unit->cell->state != SCOOP_INIT_FAILED || *unit->failure_root == NULL) {
        scoop_thread_registry_unlock();
        initialization_fatal("initialization failure read from a non-failed unit");
    }
    void *failure = *unit->failure_root;
    scoop_thread_registry_unlock();
    return failure;
}

uint64_t scoop_rt_init_enter_impl(const ScoopInitializationUnitDescriptor *unit,
                                  uintptr_t return_pc, uintptr_t stack_pointer,
                                  uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    uint64_t result = init_enter(unit);
    scoop_thread_pop_managed_anchor(&anchor);
    return result;
}

void scoop_rt_init_succeed_impl(const ScoopInitializationUnitDescriptor *unit,
                                uintptr_t return_pc, uintptr_t stack_pointer,
                                uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    init_succeed(unit);
    scoop_thread_pop_managed_anchor(&anchor);
}

void scoop_rt_init_fail_impl(const ScoopInitializationUnitDescriptor *unit,
                             void *exception, uintptr_t return_pc,
                             uintptr_t stack_pointer, uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    init_fail(unit, exception);
    scoop_thread_pop_managed_anchor(&anchor);
}

void *scoop_rt_init_failure_impl(const ScoopInitializationUnitDescriptor *unit,
                                 uintptr_t return_pc, uintptr_t stack_pointer,
                                 uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    void *failure = init_failure(unit);
    scoop_thread_pop_managed_anchor(&anchor);
    return failure;
}

const ScoopString *scoop_rt_init_cycle_message_impl(
    const ScoopInitializationUnitDescriptor *unit, uintptr_t return_pc,
    uintptr_t stack_pointer, uintptr_t frame_pointer) {
    require_unit(unit);
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    ScoopThreadState *thread = scoop_thread_current_required();
    scoop_thread_registry_lock();
    if (thread->initialization_cycle_path == NULL) {
        scoop_thread_registry_unlock();
        initialization_fatal("initialization cycle message requested without a cycle");
    }
    size_t length = strlen(thread->initialization_cycle_path);
    char *path = malloc(length + 1);
    if (path == NULL) {
        scoop_thread_registry_unlock();
        initialization_fatal("out of memory copying initialization cycle path");
    }
    memcpy(path, thread->initialization_cycle_path, length + 1);
    scoop_thread_registry_unlock();
    ScoopString *message =
        scoop_gc_alloc_internal(&scoop_td_String, sizeof(ScoopString) + length);
    message->len = (uint64_t)length;
    memcpy(message->data, path, length);
    free(path);
    scoop_thread_pop_managed_anchor(&anchor);
    return message;
}

void scoop_rt_initialize_image(void) {
    const void *gateway_boundary = __builtin_frame_address(0);
    const void *previous_boundary =
        scoop_thread_push_managed_gateway_boundary(gateway_boundary);
    const char *previous_key = NULL;
    for (uint64_t index = 0; index < scoop_image_initialization_unit_count; index++) {
        const ScoopInitializationUnitDescriptor *unit =
            &scoop_image_initialization_units[index];
        require_unit(unit);
        if (previous_key != NULL && strcmp(previous_key, unit->stable_key) >= 0) {
            initialization_fatal("initialization unit table is not in stable-key order");
        }
        if (unit->cell->state != SCOOP_INIT_UNINITIALIZED ||
            unit->cell->owner_thread != NULL || *unit->failure_root != NULL) {
            initialization_fatal("initialization unit is not pristine at startup");
        }
        for (uint64_t previous = 0; previous < index; previous++) {
            const ScoopInitializationUnitDescriptor *seen =
                &scoop_image_initialization_units[previous];
            if (seen->cell == unit->cell || seen->storage == unit->storage ||
                seen->failure_root == unit->failure_root) {
                initialization_fatal("initialization unit descriptor aliases another unit");
            }
        }
        previous_key = unit->stable_key;
    }
    for (uint64_t index = 0; index < scoop_image_initialization_unit_count; index++) {
        const ScoopInitializationUnitDescriptor *unit =
            &scoop_image_initialization_units[index];
        unit->ensure_entry();
    }
    scoop_thread_pop_managed_gateway_boundary(gateway_boundary, previous_boundary);
}
