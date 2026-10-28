#ifndef SCOOP_THROWABLE_H
#define SCOOP_THROWABLE_H

#include "scoop_rt.h"

/* The private prefix shared by Throwable and every source subclass. The
 * snapshot slot is published with atomic builtins and scanned by the GC. */
typedef struct ScoopThrowable {
    ScoopObjectHeader header;
    ScoopArray *snapshot;
} ScoopThrowable;

_Static_assert(offsetof(ScoopThrowable, snapshot) == 16, "Throwable snapshot offset");
_Static_assert(sizeof(ScoopThrowable) == 24, "Throwable inherited prefix");

#endif
