#include "eh_internal.h"

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

static struct _Unwind_Exception exception;
static unsigned cleanups;
static unsigned deletions;
extern int catch_probe(void);

static void delete_probe(_Unwind_Reason_Code reason,
                         struct _Unwind_Exception *record) {
    (void)reason;
    assert(record == &exception);
    ++deletions;
}

void throw_probe(void) {
    exception.exception_class = SCOOP_EXCEPTION_CLASS;
    exception.exception_cleanup = delete_probe;
    _Unwind_RaiseException(&exception);
    abort();
}

void cleanup_probe(void) { ++cleanups; }

void caught_probe(void *record, int selector) {
    assert(record == &exception);
    assert(selector == 1);
    _Unwind_DeleteException(record);
}

int main(void) {
    assert(catch_probe() == 42);
    assert(cleanups == 1);
    assert(deletions == 1);
    puts("SEH catch/cleanup/resume passed");
    return 0;
}
