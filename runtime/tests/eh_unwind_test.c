#include "eh_internal.h"

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

static struct _Unwind_Exception exception;
static unsigned cleanups, catches, deletions;

extern void test_catch_frame(void);

static void delete_exception(_Unwind_Reason_Code reason,
                             struct _Unwind_Exception *record) {
    assert(reason == _URC_FOREIGN_EXCEPTION_CAUGHT);
    assert(record == &exception);
    deletions++;
}

void test_throw(void) {
    exception.exception_class = SCOOP_EXCEPTION_CLASS;
    exception.exception_cleanup = delete_exception;
    (void)_Unwind_RaiseException(&exception);
    abort();
}

void test_cleanup(void) { cleanups++; }

void test_caught(void *record) {
    assert(record == &exception);
    assert(cleanups == 1);
    catches++;
    _Unwind_DeleteException(record);
}

int main(void) {
    test_catch_frame();
    assert(cleanups == 1 && catches == 1 && deletions == 1);
    puts("cleanup, catch and delete passed");
    return 0;
}
