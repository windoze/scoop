#include "eh/lsda_reader.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

static _Noreturn void scoop_eh_personality_error(const char *reason) {
    fprintf(stderr, "scoop: fatal unwind error: %s\n", reason);
    abort();
}

static _Noreturn void scoop_eh_lsda_error(ScoopEhDecodeKind kind,
                                          ScoopEhDecodeReason reason,
                                          uintptr_t region_start) {
    const char *classification = kind == SCOOP_EH_DECODE_UNSUPPORTED
                                     ? "unsupported LSDA"
                                     : "malformed LSDA";
    fprintf(stderr,
            "scoop: fatal unwind error: %s (%s) in function 0x%" PRIxPTR "\n",
            classification, scoop_eh_decode_reason_name(reason), region_start);
    abort();
}

/* Level I exposes an LSDA pointer but no section extent. A catch-all table
 * ends at its TType base; a cleanup-only table ends after its call sites.
 * Both headers delimit the range already checked in the object reader. */
static ScoopEhByteRange scoop_eh_runtime_lsda_range(const uint8_t *bytes,
                                                    uintptr_t region_start) {
    if (bytes == NULL) {
        scoop_eh_lsda_error(SCOOP_EH_DECODE_MALFORMED,
                            SCOOP_EH_REASON_NULL_RANGE, region_start);
    }
    uintptr_t address = (uintptr_t)bytes;
    if (address > UINTPTR_MAX - 13) {
        scoop_eh_lsda_error(SCOOP_EH_DECODE_MALFORMED,
                            SCOOP_EH_REASON_POINTER_OVERFLOW, region_start);
    }
    if (bytes[0] != SCOOP_EH_DW_PE_OMIT) {
        return (ScoopEhByteRange){.bytes = bytes, .length = 1};
    }
    bool has_type_table = bytes[1] != SCOOP_EH_DW_PE_OMIT;
    if (has_type_table && bytes[1] != SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT) {
        return (ScoopEhByteRange){.bytes = bytes, .length = 2};
    }
    if (!has_type_table && bytes[2] != SCOOP_EH_DW_PE_ULEB128) {
        return (ScoopEhByteRange){.bytes = bytes, .length = 3};
    }

    ScoopEhCursor cursor = {
        .range = {.bytes = bytes, .length = 13},
        .offset = has_type_table ? 2 : 3,
    };
    uint64_t delta = 0;
    ScoopEhReadStatus status = scoop_eh_read_uleb128(&cursor, &delta);
    if (status != SCOOP_EH_READ_OK) {
        scoop_eh_lsda_error(SCOOP_EH_DECODE_MALFORMED,
                            scoop_eh_read_reason(status), region_start);
    }
    size_t delta_size = 0;
    size_t length = 0;
    if (!scoop_eh_size_from_u64(delta, &delta_size) ||
        !scoop_eh_checked_size_add(cursor.offset, delta_size, &length) ||
        length > UINTPTR_MAX - address) {
        scoop_eh_lsda_error(SCOOP_EH_DECODE_MALFORMED,
                            SCOOP_EH_REASON_POINTER_OVERFLOW, region_start);
    }
    return (ScoopEhByteRange){.bytes = bytes, .length = length};
}

static _Unwind_Reason_Code
scoop_eh_install_context(struct _Unwind_Exception *exception_object,
                         struct _Unwind_Context *context, uintptr_t landing_pad,
                         uint32_t selector) {
    const int exception_register = __builtin_eh_return_data_regno(0);
    const int selector_register = __builtin_eh_return_data_regno(1);
    if (exception_register < 0 || selector_register < 0) {
        scoop_eh_personality_error(
            "target has no qualified exception data registers");
    }
    _Unwind_SetGR(context, exception_register, (uintptr_t)exception_object);
    _Unwind_SetGR(context, selector_register, (uintptr_t)selector);
    _Unwind_SetIP(context, landing_pad);
    return _URC_INSTALL_CONTEXT;
}

_Unwind_Reason_Code
scoop_eh_personality(int version, _Unwind_Action actions,
                     _Unwind_Exception_Class exception_class,
                     struct _Unwind_Exception *exception_object,
                     struct _Unwind_Context *context) {
    if (version != 1) {
        scoop_eh_personality_error("unsupported personality version");
    }
    if (exception_object == NULL || context == NULL) {
        scoop_eh_personality_error("null unwind state entered Scoop EH");
    }
    if ((uint64_t)exception_class != SCOOP_EXCEPTION_CLASS) {
        scoop_eh_personality_error("foreign exception entered Scoop EH");
    }
    if ((actions & _UA_FORCE_UNWIND) != 0) {
        scoop_eh_personality_error("forced unwind entered Scoop EH");
    }

    bool search = (actions & _UA_SEARCH_PHASE) != 0;
    bool cleanup = (actions & _UA_CLEANUP_PHASE) != 0;
    bool handler_frame = (actions & _UA_HANDLER_FRAME) != 0;
    if (search == cleanup || (search && actions != _UA_SEARCH_PHASE) ||
        (cleanup &&
         (actions & ~(_UA_CLEANUP_PHASE | _UA_HANDLER_FRAME)) != 0)) {
        scoop_eh_personality_error("invalid unwind action flags");
    }

    uintptr_t region_start = (uintptr_t)_Unwind_GetRegionStart(context);
    uintptr_t instruction_pointer = (uintptr_t)_Unwind_GetIP(context);
    const uint8_t *lsda =
        (const uint8_t *)(uintptr_t)_Unwind_GetLanguageSpecificData(context);
    ScoopEhDecodeResult decoded =
        scoop_eh_decode_lsda(scoop_eh_runtime_lsda_range(lsda, region_start),
                             region_start, instruction_pointer);
    if (decoded.kind == SCOOP_EH_DECODE_MALFORMED ||
        decoded.kind == SCOOP_EH_DECODE_UNSUPPORTED) {
        scoop_eh_lsda_error(decoded.kind, decoded.reason, region_start);
    }

    if (search) {
        return decoded.kind == SCOOP_EH_DECODE_CATCH_ALL ? _URC_HANDLER_FOUND
                                                         : _URC_CONTINUE_UNWIND;
    }
    if (handler_frame) {
        if (decoded.kind != SCOOP_EH_DECODE_CATCH_ALL ||
            decoded.selector != 1) {
            scoop_eh_personality_error(
                "handler action changed between unwind phases");
        }
        return scoop_eh_install_context(exception_object, context,
                                        decoded.landing_pad, 1);
    }
    if (decoded.kind == SCOOP_EH_DECODE_CLEANUP) {
        return scoop_eh_install_context(exception_object, context,
                                        decoded.landing_pad, 0);
    }
    return _URC_CONTINUE_UNWIND;
}
