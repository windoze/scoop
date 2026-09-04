#ifndef SCOOP_RT_EH_INTERNAL_H
#define SCOOP_RT_EH_INTERNAL_H

#include <stddef.h>
#include <stdint.h>

/* LLVM 22.1 Darwin/AArch64 LSDA encodings accepted by the Scoop personality.
 * Keep this profile closed: adding an encoding requires target qualification
 * before the decoder is extended. */
enum {
    SCOOP_EH_DW_PE_ULEB128 = 0x01,
    SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT = 0x9b,
    SCOOP_EH_DW_PE_OMIT = 0xff,
};

typedef struct ScoopEhByteRange {
    const uint8_t *bytes;
    size_t length;
} ScoopEhByteRange;

typedef enum ScoopEhDecodeKind {
    SCOOP_EH_DECODE_NO_ACTION,
    SCOOP_EH_DECODE_CLEANUP,
    SCOOP_EH_DECODE_CATCH_ALL,
    SCOOP_EH_DECODE_MALFORMED,
    SCOOP_EH_DECODE_UNSUPPORTED,
} ScoopEhDecodeKind;

typedef enum ScoopEhDecodeReason {
    SCOOP_EH_REASON_NONE,
    SCOOP_EH_REASON_NULL_RANGE,
    SCOOP_EH_REASON_TRUNCATED,
    SCOOP_EH_REASON_INTEGER_OVERFLOW,
    SCOOP_EH_REASON_POINTER_OVERFLOW,
    SCOOP_EH_REASON_IP_BEFORE_REGION,
    SCOOP_EH_REASON_EMPTY_CALL_SITE_RANGE,
    SCOOP_EH_REASON_OVERLAPPING_CALL_SITE_RANGES,
    SCOOP_EH_REASON_TYPE_TABLE_OUT_OF_RANGE,
    SCOOP_EH_REASON_TABLE_ORDER,
    SCOOP_EH_REASON_NULL_LANDING_PAD,
    SCOOP_EH_REASON_ACTION_OUT_OF_RANGE,
    SCOOP_EH_REASON_ACTION_CYCLE,
    SCOOP_EH_REASON_UNSUPPORTED_LPSTART_ENCODING,
    SCOOP_EH_REASON_UNSUPPORTED_TTYPE_ENCODING,
    SCOOP_EH_REASON_UNSUPPORTED_CALL_SITE_ENCODING,
    SCOOP_EH_REASON_UNSUPPORTED_ACTION_CHAIN,
    SCOOP_EH_REASON_UNSUPPORTED_ACTION_KIND,
    SCOOP_EH_REASON_UNSUPPORTED_TYPED_CATCH,
    SCOOP_EH_REASON_UNSUPPORTED_FILTER,
} ScoopEhDecodeReason;

typedef struct ScoopEhDecodeResult {
    ScoopEhDecodeKind kind;
    uintptr_t landing_pad;
    uint32_t selector;
    ScoopEhDecodeReason reason;
    size_t error_offset;
} ScoopEhDecodeResult;

/* Decode one LSDA inside an explicitly bounded byte range. The decoder is
 * pure: it neither consults nor mutates an unwind context. `instruction_pointer`
 * is the value reported by `_Unwind_GetIP`; ordinary frames are selected with
 * IP - 1 as required by the Itanium ABI. */
ScoopEhDecodeResult scoop_eh_decode_lsda(ScoopEhByteRange lsda,
                                         uintptr_t region_start,
                                         uintptr_t instruction_pointer);

const char *scoop_eh_decode_reason_name(ScoopEhDecodeReason reason);

#endif /* SCOOP_RT_EH_INTERNAL_H */
