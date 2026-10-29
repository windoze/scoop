#include "internal.h"

#include <stdlib.h>
#include <string.h>
#include <unwind.h>

void *scoop_trace_allocate(size_t bytes) {
    void *result = malloc(bytes);
    if (result == NULL) {
        fputs("scoop: out of memory capturing a stack trace\n", stderr);
        abort();
    }
    return result;
}

char *scoop_trace_copy_path(const char *path) {
    if (path == NULL || path[0] == 0) {
        path = "<unavailable module path>";
    }
    size_t length = strlen(path) + 1;
    char *copy = scoop_trace_allocate(length);
    memcpy(copy, path, length);
    return copy;
}

static _Unwind_Reason_Code collect_frame(struct _Unwind_Context *context, void *argument) {
    ScoopTraceFrames *frames = argument;
    int before_instruction = 0;
    uintptr_t pc = (uintptr_t)_Unwind_GetIPInfo(context, &before_instruction);
    if (pc == 0) {
        return _URC_NO_REASON;
    }
    if (frames->count == SCOOP_TRACE_MAX_FRAMES) {
        frames->flags |= SCOOP_TRACE_TRUNCATED;
        return _URC_NORMAL_STOP;
    }
    frames->frames[frames->count++] = (ScoopTraceFrame){
        .raw_pc = pc,
        .query_pc = before_instruction ? pc : pc - 1,
        .link_pc = before_instruction ? pc : pc - 1,
        .module = SCOOP_TRACE_NO_MODULE,
    };
    return _URC_NO_REASON;
}

void scoop_trace_capture_frames(ScoopTraceFrames *frames, uintptr_t caller_pc) {
    memset(frames, 0, sizeof *frames);
    _Unwind_Reason_Code reason = _Unwind_Backtrace(collect_frame, frames);
    if (reason != _URC_END_OF_STACK && reason != _URC_NO_REASON &&
        (frames->flags & SCOOP_TRACE_TRUNCATED) == 0) {
        frames->flags |= SCOOP_TRACE_PARTIAL_UNWIND;
    }
    /* Match the actual direct caller boundary, never an optimization-dependent
     * number of runtime helper frames. Keep a partial trace if it is missing. */
    for (uint32_t index = 0; index < frames->count; index++) {
        if (frames->frames[index].raw_pc == caller_pc) {
            frames->count -= index;
            memmove(frames->frames, frames->frames + index,
                    frames->count * sizeof frames->frames[0]);
            return;
        }
    }
    frames->flags |= SCOOP_TRACE_BOUNDARY_MISSING;
}

ScoopTraceBuffer scoop_trace_complete(ScoopTraceFrames *frames) {
    ScoopTraceLoadedModule modules[SCOOP_TRACE_MAX_FRAMES] = {0};
    uint32_t count = scoop_trace_modules(frames, modules);
    size_t length = sizeof(ScoopTraceHeader) + frames->count * sizeof(ScoopTraceFrame) +
                    count * sizeof(ScoopTraceModule);
    for (uint32_t index = 0; index < count; index++) {
        size_t path_length = strlen(modules[index].path);
        if (path_length >= UINT32_MAX || length > UINT32_MAX - path_length - 1) {
            fputs("scoop: stack trace module path is too long\n", stderr);
            abort();
        }
        modules[index].identity.path_offset = (uint32_t)length;
        modules[index].identity.path_length = (uint32_t)path_length;
        length += path_length + 1;
    }
    char *bytes = scoop_trace_allocate(length);
    ScoopTraceHeader header = {
        .frame_count = frames->count, .module_count = count, .flags = frames->flags};
    memcpy(bytes, &header, sizeof header);
    memcpy(bytes + sizeof header, frames->frames, frames->count * sizeof(ScoopTraceFrame));
    char *module_bytes = bytes + sizeof header + frames->count * sizeof(ScoopTraceFrame);
    for (uint32_t index = 0; index < count; index++) {
        ScoopTraceModule *module = &modules[index].identity;
        memcpy(module_bytes + index * sizeof *module, module, sizeof *module);
        memcpy(bytes + module->path_offset, modules[index].path, module->path_length + 1);
        free(modules[index].path);
    }
    return (ScoopTraceBuffer){.bytes = bytes, .length = length};
}
