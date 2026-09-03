#include "internal.h"

void scoop_rt_push_caller_roots(ScoopCallerRootFrame *frame,
                                ScoopCallerRootEntry *entries,
                                uint64_t count) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (frame == NULL) {
        scoop_thread_fatal("invalid caller root frame");
    }
    if (count != 0 && entries == NULL) {
        scoop_thread_fatal("invalid caller root frame");
    }
    for (uint64_t i = 0; i < count; i++) {
        if (entries[i].base == NULL || entries[i].scan == NULL) {
            scoop_thread_fatal("invalid caller root frame");
        }
    }
    for (ScoopCallerRootFrame *active = state->caller_roots; active != NULL;
         active = active->previous) {
        if (active == frame) {
            scoop_thread_fatal("caller root frame is already active");
        }
    }
    frame->previous = state->caller_roots;
    frame->entries = entries;
    frame->count = count;
    state->caller_roots = frame;
}
void scoop_rt_pop_caller_roots(ScoopCallerRootFrame *frame) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (frame == NULL || state->caller_roots != frame) {
        scoop_thread_fatal("caller root frames must be popped in LIFO order");
    }
    ScoopCallerRootFrame *previous = frame->previous;
    state->caller_roots = previous;
    frame->previous = NULL;
    frame->entries = NULL;
    frame->count = 0;
}

void scoop_rt_push_compiler_roots(ScoopCompilerRootFrame *frame,
                                  ScoopCallerRootEntry *entries,
                                  uint64_t count) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (frame == NULL || (count != 0 && entries == NULL)) {
        scoop_thread_fatal("invalid compiler root frame");
    }
    for (uint64_t i = 0; i < count; i++) {
        if (entries[i].base == NULL || entries[i].scan == NULL) {
            scoop_thread_fatal("invalid compiler root frame");
        }
    }
    for (ScoopCompilerRootFrame *active = state->compiler_roots; active != NULL;
         active = active->previous) {
        if (active == frame) {
            scoop_thread_fatal("compiler root frame is already active");
        }
    }
    frame->previous = state->compiler_roots;
    frame->entries = entries;
    frame->count = count;
    state->compiler_roots = frame;
}

void scoop_rt_pop_compiler_roots(ScoopCompilerRootFrame *frame) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (frame == NULL || state->compiler_roots != frame) {
        scoop_thread_fatal("compiler root frames must be popped in LIFO order");
    }
    state->compiler_roots = frame->previous;
    frame->previous = NULL;
    frame->entries = NULL;
    frame->count = 0;
}

void scoop_rt_pop_top_compiler_roots(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (state->compiler_roots == NULL) {
        scoop_thread_fatal("unwind has no active compiler root frame");
    }
    scoop_rt_pop_compiler_roots(state->compiler_roots);
}
