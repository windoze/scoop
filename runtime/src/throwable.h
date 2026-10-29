#ifndef SCOOP_THROWABLE_H
#define SCOOP_THROWABLE_H

#include "scoop_rt.h"
#include <stdio.h>

/* The private prefix shared by Throwable and every source subclass. The
 * snapshot slot is published with atomic builtins and scanned by the GC. */
typedef struct ScoopThrowable {
    ScoopObjectHeader header;
    ScoopArray *snapshot;
} ScoopThrowable;

_Static_assert(offsetof(ScoopThrowable, snapshot) == 16, "Throwable snapshot offset");
_Static_assert(sizeof(ScoopThrowable) == 24, "Throwable inherited prefix");

typedef struct ScoopThrowableReport {
    void *bytes;
    size_t capacity;
    size_t name_length;
    size_t trace_offset;
    bool captured;
} ScoopThrowableReport;

/* Copy while the rooted object is stable; a nonzero result requests capacity.
 * Reserving and rendering require only native data and may run in NativeSafe. */
size_t scoop_throwable_report_copy(const ScoopThrowable *exception, ScoopThrowableReport *report);
void scoop_throwable_report_reserve(ScoopThrowableReport *report, size_t required);
void scoop_throwable_report_render(const ScoopThrowableReport *report, FILE *stream);
void scoop_throwable_report_render_frames(const ScoopThrowableReport *report, FILE *stream);

#endif
