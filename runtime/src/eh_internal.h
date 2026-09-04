#ifndef SCOOP_RT_EH_INTERNAL_H
#define SCOOP_RT_EH_INTERNAL_H

#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <unwind.h>

#define SCOOP_EXCEPTION_CLASS UINT64_C(0x53434f4f50000000)
#define SCOOP_EXCEPTION_RECORD_MAGIC UINT64_C(0x53434f4f50454831)
#define SCOOP_EXCEPTION_RECORD_DELETED UINT64_C(0xdead53434f4f5045)

struct ScoopThreadState;

typedef enum ScoopExceptionState {
    SCOOP_EXCEPTION_ALLOCATED,
    SCOOP_EXCEPTION_IN_FLIGHT,
    SCOOP_EXCEPTION_CAUGHT,
    SCOOP_EXCEPTION_RETHROWING,
    SCOOP_EXCEPTION_DELETING,
    SCOOP_EXCEPTION_DELETED,
} ScoopExceptionState;

/* Runtime-private storage. Generated code observes only `unwind` as an
 * opaque pointer and the aligned payload returned by begin-catch. */
typedef struct ScoopExceptionRecord {
    uint64_t magic;
    ScoopExceptionState state;
    void *allocation_base;
    size_t allocation_size;
    size_t payload_offset;
    struct ScoopThreadState *owner;
    struct ScoopExceptionRecord *caught_previous;
    struct ScoopExceptionRecord *active_previous;
    struct ScoopExceptionRecord *active_next;
    struct _Unwind_Exception unwind;
    unsigned char payload_storage[];
} ScoopExceptionRecord;

_Static_assert(CHAR_BIT == 8, "the Scoop EH ABI requires 8-bit bytes");
_Static_assert(sizeof(uintptr_t) == sizeof(uint64_t),
               "the Darwin/AArch64 EH profile requires 64-bit uintptr_t");
_Static_assert(sizeof(_Unwind_Exception_Class) == sizeof(uint64_t),
               "the Scoop exception class must be exactly 64 bits");
_Static_assert(_Alignof(ScoopExceptionRecord) >=
                   _Alignof(struct _Unwind_Exception),
               "the exception record under-aligns _Unwind_Exception");
_Static_assert(offsetof(ScoopExceptionRecord, unwind) %
                       _Alignof(struct _Unwind_Exception) ==
                   0,
               "the embedded _Unwind_Exception is misaligned");
_Static_assert(offsetof(ScoopExceptionRecord, unwind) +
                       sizeof(struct _Unwind_Exception) <=
                   sizeof(ScoopExceptionRecord),
               "the unwind container conversion is invalid");

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

_Unwind_Reason_Code scoop_eh_personality(
    int version, _Unwind_Action actions,
    _Unwind_Exception_Class exception_class,
    struct _Unwind_Exception *exception_object,
    struct _Unwind_Context *context);

void scoop_eh_prepare_shutdown(void);
uint64_t scoop_eh_debug_active_record_count(void);
uint64_t scoop_eh_debug_caught_frame_count(void);

#endif /* SCOOP_RT_EH_INTERNAL_H */
