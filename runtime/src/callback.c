/* M13 managed foreign-callback token registry and invocation gateway. */
#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "scoop_rt.h"
#include "thread.h"

/* Keep cookies below bit 47 so integer -> pointer -> integer round-trips as a
 * canonical userspace address on the supported 64-bit POSIX targets. */
#define CALLBACK_SLOT_BITS 24
#define CALLBACK_SLOT_MASK ((UINT64_C(1) << CALLBACK_SLOT_BITS) - 1)
#define CALLBACK_GENERATION_MAX ((UINT32_C(1) << (47 - CALLBACK_SLOT_BITS)) - 1)

typedef struct ScoopCallbackToken {
    const void *signature;
    ScoopForeignCallbackAdapter adapter;
    uint64_t closure_handle;
    uint64_t failure_handle;
    uint64_t owners;
    uint64_t active;
    int64_t next_free;
    uint32_t generation;
    uint32_t mode;
    uint32_t state;
    bool one_shot_claimed;
    bool live;
    bool retired;
} ScoopCallbackToken;

typedef struct ReleasedHandles {
    uint64_t closure;
    uint64_t failure;
} ReleasedHandles;

static pthread_mutex_t callback_lock = PTHREAD_MUTEX_INITIALIZER;
static ScoopCallbackToken *callback_tokens;
static size_t callback_tokens_len;
static size_t callback_tokens_cap;
static int64_t callback_free = -1;
static uint64_t callback_live_count;
static bool callback_initialized;
static bool callback_shutting_down;

static _Noreturn void callback_fatal(const char *message) {
    fprintf(stderr, "scoop callback: %s\n", message);
    abort();
}

static void lock_callbacks(void) {
    if (pthread_mutex_lock(&callback_lock) != 0) {
        callback_fatal("failed to lock callback registry");
    }
}

static void unlock_callbacks(void) {
    if (pthread_mutex_unlock(&callback_lock) != 0) {
        callback_fatal("failed to unlock callback registry");
    }
}

static uintptr_t encode_cookie(size_t index, uint32_t generation) {
    uint64_t slot = (uint64_t)index + 1;
    if (slot > CALLBACK_SLOT_MASK || generation == 0 ||
        generation > CALLBACK_GENERATION_MAX) {
        callback_fatal("callback cookie payload exhausted");
    }
    return (uintptr_t)(((uint64_t)generation << CALLBACK_SLOT_BITS) | slot);
}

static ScoopCallbackToken *resolve_cookie_locked(void *context,
                                                 size_t *index_out) {
    uintptr_t cookie = (uintptr_t)context;
    uint64_t encoded_slot = cookie & CALLBACK_SLOT_MASK;
    uint32_t generation = (uint32_t)(cookie >> CALLBACK_SLOT_BITS);
    if (encoded_slot == 0 || generation == 0 ||
        generation > CALLBACK_GENERATION_MAX) {
        return NULL;
    }
    size_t index = (size_t)encoded_slot - 1;
    if (index >= callback_tokens_len) {
        return NULL;
    }
    ScoopCallbackToken *token = &callback_tokens[index];
    if (!token->live || token->generation != generation) {
        return NULL;
    }
    if (index_out != NULL) {
        *index_out = index;
    }
    return token;
}

static ScoopCallbackToken *require_cookie_locked(void *context,
                                                 size_t *index_out) {
    ScoopCallbackToken *token = resolve_cookie_locked(context, index_out);
    if (token == NULL) {
        callback_fatal("invalid or stale callback cookie");
    }
    return token;
}

static ReleasedHandles finalize_locked(size_t index,
                                       ScoopCallbackToken *token) {
    if (!token->live || token->owners != 0 || token->active != 0) {
        callback_fatal("attempted to finalize an active callback token");
    }
    ReleasedHandles handles = {
        .closure = token->closure_handle,
        .failure = token->failure_handle,
    };
    token->signature = NULL;
    token->adapter = NULL;
    token->closure_handle = 0;
    token->failure_handle = 0;
    token->one_shot_claimed = false;
    token->live = false;
    callback_live_count--;
    if (token->generation == CALLBACK_GENERATION_MAX) {
        token->retired = true;
        token->next_free = -1;
    } else {
        token->next_free = callback_free;
        callback_free = (int64_t)index;
    }
    return handles;
}

static void release_handles(ReleasedHandles handles) {
    if (handles.closure != 0) {
        (void)scoop_rt_release_handle(handles.closure);
    }
    if (handles.failure != 0) {
        (void)scoop_rt_release_handle(handles.failure);
    }
}

void scoop_callback_runtime_init(void) {
    lock_callbacks();
    if (callback_initialized || callback_tokens_len != 0 ||
        callback_live_count != 0) {
        unlock_callbacks();
        callback_fatal("callback runtime initialized more than once");
    }
    callback_initialized = true;
    callback_shutting_down = false;
    unlock_callbacks();
}

void scoop_callback_prepare_shutdown(void) {
    lock_callbacks();
    if (!callback_initialized || callback_shutting_down) {
        unlock_callbacks();
        callback_fatal("callback shutdown entered from an invalid state");
    }
    callback_shutting_down = true;
    uint64_t live = callback_live_count;
    unlock_callbacks();
    if (live != 0) {
        fprintf(stderr,
                "scoop callback: shutdown with %llu live callback token(s)\n",
                (unsigned long long)live);
        abort();
    }
}

void *scoop_runtime_callback_register(const void *closure,
                                      ScoopForeignCallbackAdapter adapter,
                                      const void *signature_descriptor,
                                      uint32_t mode) {
    if (closure == NULL || adapter == NULL || signature_descriptor == NULL ||
        (mode != SCOOP_FOREIGN_CALLBACK_REUSABLE &&
         mode != SCOOP_FOREIGN_CALLBACK_ONE_SHOT)) {
        callback_fatal("invalid callback registration");
    }
    uint64_t closure_handle = scoop_rt_get_handle(closure);
    lock_callbacks();
    if (!callback_initialized || callback_shutting_down) {
        unlock_callbacks();
        (void)scoop_rt_release_handle(closure_handle);
        callback_fatal("callback registration after shutdown began");
    }

    size_t index;
    ScoopCallbackToken *token;
    if (callback_free >= 0) {
        index = (size_t)callback_free;
        token = &callback_tokens[index];
        callback_free = token->next_free;
        token->generation++;
        if (token->generation == 0 || token->retired ||
            token->generation > CALLBACK_GENERATION_MAX) {
            unlock_callbacks();
            (void)scoop_rt_release_handle(closure_handle);
            callback_fatal("callback cookie generation exhausted");
        }
    } else {
        if (callback_tokens_len == CALLBACK_SLOT_MASK) {
            unlock_callbacks();
            (void)scoop_rt_release_handle(closure_handle);
            callback_fatal("callback cookie slots exhausted");
        }
        if (callback_tokens_len == callback_tokens_cap) {
            size_t new_cap =
                callback_tokens_cap == 0 ? 16 : callback_tokens_cap * 2;
            ScoopCallbackToken *grown =
                realloc(callback_tokens, new_cap * sizeof *grown);
            if (grown == NULL) {
                unlock_callbacks();
                (void)scoop_rt_release_handle(closure_handle);
                callback_fatal("out of memory growing callback registry");
            }
            callback_tokens = grown;
            callback_tokens_cap = new_cap;
        }
        index = callback_tokens_len++;
        token = &callback_tokens[index];
        *token = (ScoopCallbackToken){0};
        token->generation = 1;
    }
    token->signature = signature_descriptor;
    token->adapter = adapter;
    token->closure_handle = closure_handle;
    token->failure_handle = 0;
    token->owners = 1;
    token->active = 0;
    token->next_free = -1;
    token->mode = mode;
    token->state = SCOOP_FOREIGN_CALLBACK_REGISTERED;
    token->one_shot_claimed = false;
    token->live = true;
    callback_live_count++;
    uintptr_t cookie = encode_cookie(index, token->generation);
    unlock_callbacks();
    return (void *)cookie;
}

void *scoop_runtime_callback_retain(void *context) {
    lock_callbacks();
    ScoopCallbackToken *token = require_cookie_locked(context, NULL);
    if (token->owners == UINT64_MAX) {
        unlock_callbacks();
        callback_fatal("callback ownership count overflow");
    }
    token->owners++;
    unlock_callbacks();
    return context;
}

void scoop_runtime_callback_release(void *context) {
    ReleasedHandles released = {0};
    lock_callbacks();
    size_t index;
    ScoopCallbackToken *token = require_cookie_locked(context, &index);
    if (token->owners == 0) {
        unlock_callbacks();
        callback_fatal("callback ownership underflow");
    }
    token->owners--;
    if (token->owners == 0 && token->active == 0) {
        released = finalize_locked(index, token);
    }
    unlock_callbacks();
    release_handles(released);
}

uint32_t scoop_runtime_callback_state(void *context) {
    lock_callbacks();
    ScoopCallbackToken *token = require_cookie_locked(context, NULL);
    uint32_t state = token->state;
    unlock_callbacks();
    return state;
}

const void *scoop_runtime_callback_failure(void *context) {
    lock_callbacks();
    ScoopCallbackToken *token = require_cookie_locked(context, NULL);
    uint64_t failure = token->failure_handle;
    unlock_callbacks();
    return failure == 0 ? NULL : scoop_rt_resolve_handle(failure);
}

uint32_t scoop_runtime_callback_invoke(
    void *context, const void *signature_descriptor, void *result_storage,
    const void *const *argument_storage) {
    ScoopForeignCallbackAdapter adapter;
    uint64_t closure_handle;
    uint32_t mode;

    lock_callbacks();
    ScoopCallbackToken *token = require_cookie_locked(context, NULL);
    if (token->signature != signature_descriptor) {
        unlock_callbacks();
        callback_fatal("callback signature descriptor mismatch");
    }
    mode = token->mode;
    if (mode == SCOOP_FOREIGN_CALLBACK_ONE_SHOT) {
        if (token->one_shot_claimed || token->owners == 0) {
            unlock_callbacks();
            callback_fatal("one-shot callback invoked more than once");
        }
        token->one_shot_claimed = true;
        token->owners--; /* worker ownership becomes the active lease */
    }
    if (token->active == UINT64_MAX) {
        unlock_callbacks();
        callback_fatal("callback active count overflow");
    }
    token->active++;
    if (token->state != SCOOP_FOREIGN_CALLBACK_FAILED) {
        token->state = SCOOP_FOREIGN_CALLBACK_ACTIVE;
    }
    adapter = token->adapter;
    closure_handle = token->closure_handle;
    unlock_callbacks();

    bool attached_here = scoop_rt_attach_foreign_thread();
    ScoopCallbackThreadEntry entry = {0};
    volatile char managed_stack_boundary = 0;
    scoop_thread_enter_callback(&entry,
                                 (const void *)&managed_stack_boundary);

    const void *closure = scoop_rt_resolve_handle(closure_handle);
    void *exception = NULL;
    void **root_slots[] = {(void **)&closure, &exception};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, root_slots, 2);
    uint64_t status =
        adapter(closure, result_storage, argument_storage, &exception);
    uint64_t failure_handle = 0;
    if (status == SCOOP_FOREIGN_CALLBACK_THREW) {
        if (exception == NULL) {
            callback_fatal("callback adapter reported an empty exception");
        }
        failure_handle = scoop_rt_get_handle(exception);
    } else if (status != SCOOP_FOREIGN_CALLBACK_RETURNED) {
        callback_fatal("callback adapter returned an invalid status");
    }
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_leave_callback(&entry);
    if (attached_here) {
        scoop_rt_detach_foreign_thread();
    }

    ReleasedHandles released = {0};
    uint64_t discarded_failure = 0;
    lock_callbacks();
    size_t index;
    token = require_cookie_locked(context, &index);
    if (failure_handle != 0) {
        if (token->failure_handle == 0) {
            token->failure_handle = failure_handle;
        } else {
            discarded_failure = failure_handle;
        }
    }
    if (token->active == 0) {
        unlock_callbacks();
        callback_fatal("callback active count underflow");
    }
    token->active--;
    if (token->failure_handle != 0) {
        token->state = SCOOP_FOREIGN_CALLBACK_FAILED;
    } else if (mode == SCOOP_FOREIGN_CALLBACK_ONE_SHOT) {
        token->state = SCOOP_FOREIGN_CALLBACK_COMPLETED;
    } else if (token->active == 0) {
        token->state = SCOOP_FOREIGN_CALLBACK_REGISTERED;
    } else {
        token->state = SCOOP_FOREIGN_CALLBACK_ACTIVE;
    }
    if (token->owners == 0 && token->active == 0) {
        released = finalize_locked(index, token);
    }
    unlock_callbacks();
    if (discarded_failure != 0) {
        (void)scoop_rt_release_handle(discarded_failure);
    }
    release_handles(released);
    return status;
}

uint64_t scoop_runtime_callback_debug_live_count(void) {
    lock_callbacks();
    uint64_t count = callback_live_count;
    unlock_callbacks();
    return count;
}
