#include "eh_internal.h"
#include "throwable.h"
#include "process.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "generated_entries.h"
#include "scoop_rt.h"
#include "thread.h"

static _Noreturn void scoop_eh_state_error(const char *reason) {
    fprintf(stderr, "scoop: fatal exception state error: %s\n", reason);
    abort();
}

static void scoop_eh_register_record(ScoopThreadState *thread, ScoopExceptionRecord *record) {
    record->active_previous = NULL;
    record->active_next = thread->active_exceptions;
    if (record->active_next != NULL) {
        record->active_next->active_previous = record;
    }
    thread->active_exceptions = record;
}

static void scoop_eh_unregister_record(ScoopThreadState *thread, ScoopExceptionRecord *record) {
    if (record->active_previous != NULL) {
        record->active_previous->active_next = record->active_next;
    } else if (thread->active_exceptions == record) {
        thread->active_exceptions = record->active_next;
    } else {
        scoop_eh_state_error("active record links are corrupt");
    }
    if (record->active_next != NULL) {
        record->active_next->active_previous = record->active_previous;
    }
}

static ScoopExceptionRecord *scoop_eh_record_from_unwind(struct _Unwind_Exception *unwind) {
    if (unwind == NULL) {
        scoop_eh_state_error("invalid Scoop unwind record");
    }
    const uintptr_t unwind_address = (uintptr_t)unwind;
    const size_t unwind_offset = offsetof(ScoopExceptionRecord, unwind);
    if (unwind_address < unwind_offset) {
        scoop_eh_state_error("unwind record address underflow");
    }
    if ((uint64_t)unwind->exception_class != SCOOP_EXCEPTION_CLASS) {
        scoop_eh_state_error("foreign exception entered Scoop catch");
    }
    return (ScoopExceptionRecord *)(unwind_address - unwind_offset);
}

static ScoopExceptionRecord *scoop_eh_allocate_record(const void *object, ScoopThreadState *owner) {
    const ScoopObjectHeader *header = object;
    if (header->td->instance_shape.instance_kind != SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1) {
        scoop_eh_state_error("throw source must be a fixed object");
    }
    ScoopExceptionRecord *record = calloc(1, sizeof *record);
    if (record == NULL) {
        scoop_eh_state_error("out of memory allocating an exception record");
    }
    record->state = SCOOP_EXCEPTION_IN_FLIGHT;
    record->object = (void *)object;
    record->owner = owner;
    record->unwind.exception_class = (_Unwind_Exception_Class)SCOOP_EXCEPTION_CLASS;
    return record;
}

static void scoop_eh_pop_caught(ScoopThreadState *thread, ScoopExceptionRecord *record) {
    if (thread->caught_exception_top != record) {
        scoop_eh_state_error("catch frames ended out of LIFO order");
    }
    thread->caught_exception_top = record->caught_previous;
    record->caught_previous = NULL;
}

static void scoop_eh_exception_cleanup(_Unwind_Reason_Code reason,
                                       struct _Unwind_Exception *unwind) {
    (void)reason;
    ScoopExceptionRecord *record = scoop_eh_record_from_unwind(unwind);
    ScoopThreadState *thread = scoop_thread_current();
    if (thread == NULL || record->owner != thread || record->state != SCOOP_EXCEPTION_DELETING ||
        record->caught_previous != NULL || thread->caught_exception_top == record) {
        scoop_eh_state_error("cleanup observed a live or foreign catch frame");
    }

    scoop_eh_unregister_record(thread, record);
    free(record);
}

static const char *scoop_eh_reason_name(_Unwind_Reason_Code reason) {
    switch (reason) {
    case _URC_NO_REASON:
        return "no reason";
    case _URC_FOREIGN_EXCEPTION_CAUGHT:
        return "foreign exception caught";
    case _URC_FATAL_PHASE2_ERROR:
        return "fatal phase 2 error";
    case _URC_FATAL_PHASE1_ERROR:
        return "fatal phase 1 error";
    case _URC_NORMAL_STOP:
        return "normal stop";
    case _URC_END_OF_STACK:
        return "end of stack";
    case _URC_HANDLER_FOUND:
        return "handler found";
    case _URC_INSTALL_CONTEXT:
        return "install context";
    case _URC_CONTINUE_UNWIND:
        return "continue unwind";
    }
    return "unknown reason";
}

static _Noreturn void scoop_eh_finish_failed_raise(ScoopExceptionRecord *record,
                                                   _Unwind_Reason_Code reason, const char *phase) {
    if (record->state != SCOOP_EXCEPTION_IN_FLIGHT || record->caught_previous != NULL) {
        scoop_eh_state_error("failed raise left a corrupt exception record");
    }
    ScoopThrowableReport report = {0};
    size_t required;
    while ((required = scoop_throwable_report_copy(record->object, &report)) != 0) {
        scoop_throwable_report_reserve(&report, required);
    }
    record->state = SCOOP_EXCEPTION_DELETING;
    _Unwind_DeleteException(&record->unwind);
    scoop_process_prepare_termination();
    if (reason == _URC_END_OF_STACK) {
        fputs("scoop: uncaught exception: ", stderr);
    } else {
        fprintf(stderr, "scoop: fatal unwind error: %s: %s\n", phase, scoop_eh_reason_name(reason));
    }
    scoop_throwable_report_render(&report, stderr);
    free(report.bytes);
    abort();
}

_Noreturn void scoop_rt_throw(const void *object) {
    ScoopThreadState *thread = scoop_thread_require_managed();
    if (object == NULL) {
        scoop_eh_state_error("throw source is null");
    }

    ScoopExceptionRecord *record = scoop_eh_allocate_record(object, thread);
    record->unwind.exception_cleanup = scoop_eh_exception_cleanup;
    scoop_eh_register_record(thread, record);

    _Unwind_Reason_Code reason = _Unwind_RaiseException(&record->unwind);
    scoop_eh_finish_failed_raise(record, reason, "raise");
}

void *scoop_rt_begin_catch(void *raw_exception) {
    ScoopThreadState *thread = scoop_thread_require_managed();
    ScoopExceptionRecord *record = scoop_eh_record_from_unwind(raw_exception);
    if (record->owner != thread || record->state != SCOOP_EXCEPTION_IN_FLIGHT ||
        record->caught_previous != NULL) {
        scoop_eh_state_error("begin-catch observed an invalid record state");
    }
    record->caught_previous = thread->caught_exception_top;
    record->state = SCOOP_EXCEPTION_CAUGHT;
    thread->caught_exception_top = record;
    return record->object;
}

void scoop_rt_end_catch(void) {
    ScoopThreadState *thread = scoop_thread_require_managed();
    ScoopExceptionRecord *record = thread->caught_exception_top;
    if (record == NULL || record->owner != thread) {
        scoop_eh_state_error("end-catch has no active catch frame");
    }

    if (record->state == SCOOP_EXCEPTION_RETHROWING) {
        scoop_eh_pop_caught(thread, record);
        record->state = SCOOP_EXCEPTION_IN_FLIGHT;
        return;
    }
    if (record->state != SCOOP_EXCEPTION_CAUGHT) {
        scoop_eh_state_error("end-catch observed an invalid record state");
    }
    scoop_eh_pop_caught(thread, record);
    record->state = SCOOP_EXCEPTION_DELETING;
    _Unwind_DeleteException(&record->unwind);
}

_Noreturn void scoop_rt_rethrow(void) {
    ScoopThreadState *thread = scoop_thread_require_managed();
    ScoopExceptionRecord *record = thread->caught_exception_top;
    if (record == NULL || record->owner != thread || record->state != SCOOP_EXCEPTION_CAUGHT) {
        scoop_eh_state_error("rethrow has no current caught exception");
    }
    record->state = SCOOP_EXCEPTION_RETHROWING;
    _Unwind_Reason_Code reason = _Unwind_RaiseException(&record->unwind);

    if (thread->caught_exception_top == record) {
        if (record->state != SCOOP_EXCEPTION_RETHROWING) {
            scoop_eh_state_error("failed rethrow changed its catch state");
        }
        scoop_eh_pop_caught(thread, record);
        record->state = SCOOP_EXCEPTION_IN_FLIGHT;
    } else if (record->state != SCOOP_EXCEPTION_IN_FLIGHT || record->caught_previous != NULL) {
        scoop_eh_state_error("failed rethrow left a corrupt catch stack");
    }
    scoop_eh_finish_failed_raise(record, reason, "rethrow");
}

void scoop_eh_prepare_shutdown(void) {
    ScoopThreadState *thread = scoop_thread_current_required();
    if (thread->caught_exception_top != NULL || thread->active_exceptions != NULL) {
        scoop_eh_state_error("runtime shutdown with active exception records");
    }
}
