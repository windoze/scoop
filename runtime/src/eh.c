#include "eh_internal.h"

#include <inttypes.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gc/gc_internal.h"
#include "generated_entries.h"
#include "managed_entries.h"
#include "thread.h"

static pthread_mutex_t active_records_lock = PTHREAD_MUTEX_INITIALIZER;
static ScoopExceptionRecord *active_records;
static uint64_t active_record_count;
static _Atomic(uint64_t) caught_frame_count;

static _Noreturn void scoop_eh_state_error(const char *reason) {
    fprintf(stderr, "scoop: fatal exception state error: %s\n", reason);
    abort();
}

static void scoop_eh_lock_active_records(void) {
    if (pthread_mutex_lock(&active_records_lock) != 0) {
        scoop_eh_state_error("failed to lock the active record registry");
    }
}

static void scoop_eh_unlock_active_records(void) {
    if (pthread_mutex_unlock(&active_records_lock) != 0) {
        scoop_eh_state_error("failed to unlock the active record registry");
    }
}

static bool scoop_eh_record_is_active_locked(
    const ScoopExceptionRecord *record) {
    for (const ScoopExceptionRecord *current = active_records;
         current != NULL; current = current->active_next) {
        if (current == record) {
            return true;
        }
    }
    return false;
}

static bool scoop_eh_record_is_active(const ScoopExceptionRecord *record) {
    scoop_eh_lock_active_records();
    bool active = scoop_eh_record_is_active_locked(record);
    scoop_eh_unlock_active_records();
    return active;
}

static void scoop_eh_register_record(ScoopExceptionRecord *record) {
    scoop_eh_lock_active_records();
    if (scoop_eh_record_is_active_locked(record) ||
        active_record_count == UINT64_MAX) {
        scoop_eh_unlock_active_records();
        scoop_eh_state_error("active record registry is corrupt");
    }
    record->active_previous = NULL;
    record->active_next = active_records;
    if (active_records != NULL) {
        active_records->active_previous = record;
    }
    active_records = record;
    active_record_count++;
    scoop_eh_unlock_active_records();
}

static void scoop_eh_unregister_record(ScoopExceptionRecord *record) {
    scoop_eh_lock_active_records();
    if (!scoop_eh_record_is_active_locked(record) ||
        active_record_count == 0) {
        scoop_eh_unlock_active_records();
        scoop_eh_state_error("deleted an inactive exception record");
    }
    if (record->active_previous != NULL) {
        record->active_previous->active_next = record->active_next;
    } else if (active_records == record) {
        active_records = record->active_next;
    } else {
        scoop_eh_unlock_active_records();
        scoop_eh_state_error("active record links are corrupt");
    }
    if (record->active_next != NULL) {
        record->active_next->active_previous = record->active_previous;
    }
    record->active_previous = NULL;
    record->active_next = NULL;
    active_record_count--;
    scoop_eh_unlock_active_records();
}

static void *scoop_eh_payload(ScoopExceptionRecord *record) {
    if (record->payload_offset < sizeof *record ||
        record->payload_offset > record->allocation_size) {
        scoop_eh_state_error("exception payload offset is corrupt");
    }
    return (unsigned char *)record + record->payload_offset;
}

static const void *scoop_eh_const_payload(
    const ScoopExceptionRecord *record) {
    return scoop_eh_payload((ScoopExceptionRecord *)record);
}

static ScoopExceptionRecord *scoop_eh_record_from_unwind(
    struct _Unwind_Exception *unwind) {
    if (unwind == NULL) {
        scoop_eh_state_error("invalid Scoop unwind record");
    }
    const uintptr_t unwind_address = (uintptr_t)unwind;
    const size_t unwind_offset = offsetof(ScoopExceptionRecord, unwind);
    if (unwind_address < unwind_offset) {
        scoop_eh_state_error("unwind record address underflow");
    }
    ScoopExceptionRecord *record =
        (ScoopExceptionRecord *)(unwind_address - unwind_offset);
    if (!scoop_eh_record_is_active(record)) {
        scoop_eh_state_error("unwind record is not active");
    }
    /* Membership is checked before dereferencing the candidate record. This
     * keeps a repeated cleanup callback deterministic instead of reading an
     * allocation that the first cleanup already freed. */
    if ((uint64_t)unwind->exception_class != SCOOP_EXCEPTION_CLASS ||
        record->magic != SCOOP_EXCEPTION_RECORD_MAGIC ||
        &record->unwind != unwind) {
        scoop_eh_state_error("exception record identity is corrupt");
    }
    return record;
}

static bool scoop_eh_checked_add(size_t left, size_t right,
                                 size_t *result) {
    if (right > SIZE_MAX - left) {
        return false;
    }
    *result = left + right;
    return true;
}

static bool scoop_eh_power_of_two(size_t value) {
    return value != 0 && (value & (value - 1)) == 0;
}

static ScoopExceptionRecord *scoop_eh_allocate_record(
    const void *object, ScoopThreadState *owner) {
    const ScoopObjectHeader *header = object;
    const ScoopTypeDescriptor *td = header->td;
    if (td == NULL ||
        td->instance_shape.instance_kind != SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1 ||
        td->instance_shape.inline_storage_kind != SCOOP_INLINE_STORAGE_NONE_V1 ||
        td->instance_shape.minimum_size < sizeof(ScoopObjectHeader) ||
        td->instance_shape.minimum_size > SIZE_MAX ||
        td->instance_shape.instance_alignment > SIZE_MAX) {
        scoop_eh_state_error("throw source has an invalid TypeDescriptor");
    }
    size_t object_size = (size_t)td->instance_shape.minimum_size;
    size_t alignment = (size_t)td->instance_shape.instance_alignment;
    if (!scoop_eh_power_of_two(alignment) ||
        alignment < _Alignof(ScoopObjectHeader)) {
        scoop_eh_state_error("throw source has an invalid object alignment");
    }

    size_t allocation_size = 0;
    if (!scoop_eh_checked_add(sizeof(ScoopExceptionRecord), alignment - 1,
                              &allocation_size) ||
        !scoop_eh_checked_add(allocation_size, object_size,
                              &allocation_size)) {
        scoop_eh_state_error("exception record size overflow");
    }
    void *allocation = malloc(allocation_size);
    if (allocation == NULL) {
        scoop_eh_state_error("out of memory allocating an exception record");
    }
    ScoopExceptionRecord *record = allocation;
    memset(record, 0, sizeof *record);

    if ((uintptr_t)record > UINTPTR_MAX - sizeof *record) {
        free(allocation);
        scoop_eh_state_error("exception payload address overflow");
    }
    uintptr_t unaligned = (uintptr_t)record + sizeof *record;
    uintptr_t padding = (alignment - (unaligned & (alignment - 1))) &
                        (alignment - 1);
    if (padding > UINTPTR_MAX - unaligned) {
        free(allocation);
        scoop_eh_state_error("exception payload address overflow");
    }
    uintptr_t payload_address = unaligned + padding;
    size_t payload_offset = (size_t)(payload_address - (uintptr_t)record);
    if (payload_offset > allocation_size ||
        object_size > allocation_size - payload_offset) {
        free(allocation);
        scoop_eh_state_error("exception payload range is corrupt");
    }

    record->magic = SCOOP_EXCEPTION_RECORD_MAGIC;
    record->state = SCOOP_EXCEPTION_ALLOCATED;
    record->allocation_base = allocation;
    record->allocation_size = allocation_size;
    record->payload_offset = payload_offset;
    record->owner = owner;
    record->unwind.exception_class =
        (_Unwind_Exception_Class)SCOOP_EXCEPTION_CLASS;
    memcpy((void *)payload_address, object, object_size);
    return record;
}

static void scoop_eh_increment_caught_count(void) {
    uint64_t current =
        atomic_load_explicit(&caught_frame_count, memory_order_relaxed);
    for (;;) {
        if (current == UINT64_MAX) {
            scoop_eh_state_error("caught frame count overflow");
        }
        if (atomic_compare_exchange_weak_explicit(
                &caught_frame_count, &current, current + 1,
                memory_order_relaxed, memory_order_relaxed)) {
            return;
        }
    }
}

static void scoop_eh_decrement_caught_count(void) {
    uint64_t current =
        atomic_load_explicit(&caught_frame_count, memory_order_relaxed);
    for (;;) {
        if (current == 0) {
            scoop_eh_state_error("caught frame count underflow");
        }
        if (atomic_compare_exchange_weak_explicit(
                &caught_frame_count, &current, current - 1,
                memory_order_relaxed, memory_order_relaxed)) {
            return;
        }
    }
}

static void scoop_eh_pop_caught(ScoopThreadState *thread,
                                ScoopExceptionRecord *record) {
    if (thread->caught_exception_top != record) {
        scoop_eh_state_error("catch frames ended out of LIFO order");
    }
    thread->caught_exception_top = record->caught_previous;
    record->caught_previous = NULL;
    scoop_eh_decrement_caught_count();
}

static void scoop_eh_exception_cleanup(
    _Unwind_Reason_Code reason, struct _Unwind_Exception *unwind) {
    (void)reason;
    ScoopExceptionRecord *record = scoop_eh_record_from_unwind(unwind);
    ScoopThreadState *thread = scoop_thread_current();
    if (thread == NULL || record->owner != thread ||
        record->state != SCOOP_EXCEPTION_DELETING ||
        record->caught_previous != NULL ||
        thread->caught_exception_top == record) {
        scoop_eh_state_error("cleanup observed a live or foreign catch frame");
    }

    void *payload = scoop_eh_payload(record);
    scoop_eh_unregister_record(record);
    scoop_rt_gc_remove_root_object(payload);
    void *allocation = record->allocation_base;
    record->state = SCOOP_EXCEPTION_DELETED;
    record->magic = SCOOP_EXCEPTION_RECORD_DELETED;
    record->allocation_base = NULL;
    free(allocation);
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

static _Noreturn void scoop_eh_finish_failed_raise(
    ScoopExceptionRecord *record, _Unwind_Reason_Code reason,
    const char *phase) {
    if (record->state != SCOOP_EXCEPTION_IN_FLIGHT ||
        record->caught_previous != NULL || !scoop_eh_record_is_active(record)) {
        scoop_eh_state_error("failed raise left a corrupt exception record");
    }
    const ScoopObjectHeader *payload = scoop_eh_const_payload(record);
    if (reason == _URC_END_OF_STACK) {
        fputs("scoop: uncaught exception: ", stderr);
        if (payload->td != NULL && payload->td->diagnostic_name.data != NULL &&
            payload->td->diagnostic_name.length != 0 &&
            payload->td->diagnostic_name.length <= SIZE_MAX) {
            fwrite(payload->td->diagnostic_name.data, 1,
                   (size_t)payload->td->diagnostic_name.length, stderr);
        } else {
            fputs("<unknown>", stderr);
        }
        fputc('\n', stderr);
    } else {
        fprintf(stderr, "scoop: fatal unwind error: %s: %s\n", phase,
                scoop_eh_reason_name(reason));
    }
    record->state = SCOOP_EXCEPTION_DELETING;
    _Unwind_DeleteException(&record->unwind);
    abort();
}

_Noreturn void scoop_rt_throw(const void *object) {
    scoop_thread_require_managed();
    ScoopThreadState *thread = scoop_thread_current_required();
    if (!scoop_gc_is_published_object(object)) {
        scoop_eh_state_error("throw source is not a published Scoop object");
    }

    ScoopExceptionRecord *record =
        scoop_eh_allocate_record(object, thread);
    record->unwind.exception_cleanup = scoop_eh_exception_cleanup;
    void *payload = scoop_eh_payload(record);
    scoop_rt_gc_add_root_object(payload);
    record->state = SCOOP_EXCEPTION_IN_FLIGHT;
    scoop_eh_register_record(record);

    _Unwind_Reason_Code reason = _Unwind_RaiseException(&record->unwind);
    scoop_eh_finish_failed_raise(record, reason, "raise");
}

void *scoop_rt_begin_catch(void *raw_exception) {
    scoop_thread_require_managed();
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopExceptionRecord *record = scoop_eh_record_from_unwind(raw_exception);
    if (record->owner != thread || record->state != SCOOP_EXCEPTION_IN_FLIGHT ||
        record->caught_previous != NULL) {
        scoop_eh_state_error("begin-catch observed an invalid record state");
    }
    for (ScoopExceptionRecord *current = thread->caught_exception_top;
         current != NULL; current = current->caught_previous) {
        if (current == record) {
            scoop_eh_state_error("exception record was caught twice");
        }
    }
    record->caught_previous = thread->caught_exception_top;
    record->state = SCOOP_EXCEPTION_CAUGHT;
    thread->caught_exception_top = record;
    scoop_eh_increment_caught_count();
    return scoop_eh_payload(record);
}

void scoop_rt_end_catch(void) {
    scoop_thread_require_managed();
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopExceptionRecord *record = thread->caught_exception_top;
    if (record == NULL || !scoop_eh_record_is_active(record) ||
        record->owner != thread) {
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
    scoop_thread_require_managed();
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopExceptionRecord *record = thread->caught_exception_top;
    if (record == NULL || !scoop_eh_record_is_active(record) ||
        record->owner != thread || record->state != SCOOP_EXCEPTION_CAUGHT) {
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
    } else if (record->state != SCOOP_EXCEPTION_IN_FLIGHT ||
               record->caught_previous != NULL) {
        scoop_eh_state_error("failed rethrow left a corrupt catch stack");
    }
    scoop_eh_finish_failed_raise(record, reason, "rethrow");
}

void *scoop_rt_materialize_exception_impl(const void *caught,
                                          uintptr_t return_pc,
                                          uintptr_t stack_pointer,
                                          uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    ScoopThreadState *thread = scoop_thread_current_required();
    ScoopExceptionRecord *record = thread->caught_exception_top;
    if (record == NULL || !scoop_eh_record_is_active(record) ||
        record->owner != thread || record->state != SCOOP_EXCEPTION_CAUGHT ||
        scoop_eh_const_payload(record) != caught) {
        scoop_eh_state_error(
            "exception materialization has no matching catch frame");
    }

    const ScoopObjectHeader *source = scoop_eh_const_payload(record);
    const ScoopTypeDescriptor *td = source->td;
    size_t size = (size_t)td->instance_shape.minimum_size;
    void *managed = scoop_gc_alloc_internal(td, size);
    /* Allocation may move every outbound reference stored in the stable
     * external payload. Re-read that payload only after the safepoint. */
    source = scoop_eh_const_payload(record);
    if (size > sizeof(ScoopObjectHeader)) {
        memcpy((unsigned char *)managed + sizeof(ScoopObjectHeader),
               (const unsigned char *)source + sizeof(ScoopObjectHeader),
               size - sizeof(ScoopObjectHeader));
    }
    scoop_thread_pop_managed_anchor(&anchor);
    return managed;
}

void scoop_eh_prepare_shutdown(void) {
    ScoopThreadState *thread = scoop_thread_current_required();
    uint64_t caught =
        atomic_load_explicit(&caught_frame_count, memory_order_relaxed);
    scoop_eh_lock_active_records();
    uint64_t active = active_record_count;
    bool empty = active_records == NULL;
    scoop_eh_unlock_active_records();
    if (thread->caught_exception_top != NULL || caught != 0 || active != 0 ||
        !empty) {
        fprintf(stderr,
                "scoop: fatal exception state error: runtime shutdown with "
                "%" PRIu64 " active record(s) and %" PRIu64
                " caught frame(s)\n",
                active, caught);
        abort();
    }
}

uint64_t scoop_eh_debug_active_record_count(void) {
    scoop_eh_lock_active_records();
    uint64_t count = active_record_count;
    scoop_eh_unlock_active_records();
    return count;
}

uint64_t scoop_eh_debug_caught_frame_count(void) {
    return atomic_load_explicit(&caught_frame_count, memory_order_relaxed);
}
