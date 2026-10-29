#ifndef SCOOP_STACK_TRACE_H
#define SCOOP_STACK_TRACE_H

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

/* A diagnostic truncation bound, unrelated to program validity. */
#define SCOOP_TRACE_MAX_FRAMES 128
#define SCOOP_TRACE_MAX_ID 64
#define SCOOP_TRACE_NO_MODULE UINT32_MAX

enum {
    SCOOP_TRACE_TRUNCATED = 1,
    SCOOP_TRACE_PARTIAL_UNWIND = 2,
    SCOOP_TRACE_BOUNDARY_MISSING = 4
};

typedef struct ScoopTraceFrame {
    uintptr_t raw_pc;
    uintptr_t query_pc;
    uintptr_t link_pc;
    uint32_t module;
    uint32_t reserved;
} ScoopTraceFrame;

typedef struct ScoopTraceFrames {
    uint32_t count;
    uint32_t flags;
    ScoopTraceFrame frames[SCOOP_TRACE_MAX_FRAMES];
} ScoopTraceFrames;

typedef struct ScoopTraceBuffer {
    void *bytes;
    size_t length;
} ScoopTraceBuffer;

/* Capture before leaving a terminating managed frame. Completion and rendering
 * only consume native data and may acquire loader/file/stdio locks. */
void scoop_trace_capture_frames(ScoopTraceFrames *frames, uintptr_t caller_pc);
ScoopTraceBuffer scoop_trace_complete(ScoopTraceFrames *frames);
void scoop_trace_render(const void *bytes, FILE *stream);
void *scoop_trace_allocate(size_t bytes);

#endif
