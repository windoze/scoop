#define _POSIX_C_SOURCE 200809L
#include "../stack_trace.h"
#include "../throwable.h"
#include "../value_shape.h"
#include <stdlib.h>
#include <string.h>

static ScoopByteSpanV1 exception_name(const ScoopThrowable *exception) {
    ScoopByteSpanV1 name = exception->header.td->diagnostic_name;
    if (name.data == NULL || name.length == 0) {
        static const unsigned char unknown[] = "<unknown exception>";
        name = (ScoopByteSpanV1){.data = unknown, .length = sizeof unknown - 1};
    }
    return name;
}

size_t scoop_throwable_report_copy(const ScoopThrowable *exception, ScoopThrowableReport *report) {
    ScoopByteSpanV1 name = exception_name(exception);
    const ScoopArray *snapshot = __atomic_load_n(&exception->snapshot, __ATOMIC_ACQUIRE);
    size_t trace_length = snapshot == NULL ? 0 : snapshot->size;
    size_t trace_offset = (scoop_shape_add(name.length, 7)) & ~(size_t)7;
    size_t required = scoop_shape_add(trace_offset, trace_length);
    if (required > report->capacity)
        return required;
    memcpy(report->bytes, name.data, name.length);
    if (snapshot != NULL) {
        memcpy((char *)report->bytes + trace_offset, snapshot->elements, trace_length);
    }
    report->name_length = name.length;
    report->trace_offset = trace_offset;
    report->captured = snapshot != NULL;
    return 0;
}

void scoop_throwable_report_reserve(ScoopThrowableReport *report, size_t required) {
    free(report->bytes);
    report->bytes = scoop_trace_allocate(required);
    report->capacity = required;
}

void scoop_throwable_report_render(const ScoopThrowableReport *report, FILE *stream) {
    flockfile(stream);
    fwrite(report->bytes, 1, report->name_length, stream);
    fputc('\n', stream);
    scoop_throwable_report_render_frames(report, stream);
    funlockfile(stream);
}

void scoop_throwable_report_render_frames(const ScoopThrowableReport *report, FILE *stream) {
    if (report->captured) {
        scoop_trace_render((const char *)report->bytes + report->trace_offset, stream);
    } else {
        fputs("  <stack not captured: this object has not been thrown>\n", stream);
    }
}
