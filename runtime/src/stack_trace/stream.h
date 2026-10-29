#ifndef SCOOP_TRACE_STREAM_H
#define SCOOP_TRACE_STREAM_H

#include <stdio.h>

static inline void scoop_trace_lock_stream(FILE *stream) {
#if defined(_WIN32)
    _lock_file(stream);
#else
    flockfile(stream);
#endif
}

static inline void scoop_trace_unlock_stream(FILE *stream) {
#if defined(_WIN32)
    _unlock_file(stream);
#else
    funlockfile(stream);
#endif
}

#endif
