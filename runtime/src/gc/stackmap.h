#ifndef SCOOP_GC_STACKMAP_H
#define SCOOP_GC_STACKMAP_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Platform-neutral decoded form of LLVM stack map v3. The parser preserves
 * every location kind; a target profile validates which kinds are legal GC
 * roots and resolves them only while a managed frame is stopped. */
typedef enum ScoopStackMapLocationKind {
    SCOOP_STACKMAP_REGISTER = 1,
    SCOOP_STACKMAP_DIRECT = 2,
    SCOOP_STACKMAP_INDIRECT = 3,
    SCOOP_STACKMAP_CONSTANT = 4,
    SCOOP_STACKMAP_CONSTANT_INDEX = 5,
} ScoopStackMapLocationKind;

typedef struct ScoopStackMapLocation {
    ScoopStackMapLocationKind kind;
    uint16_t size;
    uint16_t dwarf_register;
    int32_t offset;
    uint64_t constant;
} ScoopStackMapLocation;

typedef struct ScoopStackMapRootPair {
    ScoopStackMapLocation base;
    ScoopStackMapLocation derived;
} ScoopStackMapRootPair;

typedef struct ScoopStackMapRecord {
    uint64_t safepoint_id;
    uintptr_t function_address;
    uintptr_t return_pc;
    uint64_t stack_size;
    ScoopStackMapRootPair *roots;
    uint16_t root_count;
} ScoopStackMapRecord;

typedef struct ScoopStackMapIndex {
    ScoopStackMapRecord *records;
    size_t record_count;
} ScoopStackMapIndex;

/* One loaded image's dyld-fixed section and executable text interval. */
typedef struct ScoopStackMapImage {
    const uint8_t *section;
    size_t section_size;
    uintptr_t text_start;
    uintptr_t text_end;
} ScoopStackMapImage;

typedef enum ScoopStackMapErrorCode {
    SCOOP_STACKMAP_OK = 0,
    SCOOP_STACKMAP_INVALID_ARGUMENT,
    SCOOP_STACKMAP_TRUNCATED,
    SCOOP_STACKMAP_UNSUPPORTED_VERSION,
    SCOOP_STACKMAP_NONZERO_RESERVED,
    SCOOP_STACKMAP_INTEGER_OVERFLOW,
    SCOOP_STACKMAP_OUT_OF_MEMORY,
    SCOOP_STACKMAP_INVALID_FUNCTION_COUNT,
    SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS,
    SCOOP_STACKMAP_INVALID_STACK_SIZE,
    SCOOP_STACKMAP_INVALID_RECORD,
    SCOOP_STACKMAP_INVALID_LOCATION,
    SCOOP_STACKMAP_INVALID_CONSTANT_INDEX,
    SCOOP_STACKMAP_DEOPT_UNSUPPORTED,
    SCOOP_STACKMAP_DUPLICATE_SAFEPOINT_ID,
    SCOOP_STACKMAP_DUPLICATE_RETURN_PC,
    SCOOP_STACKMAP_TRAILING_DATA,
} ScoopStackMapErrorCode;

typedef struct ScoopStackMapError {
    ScoopStackMapErrorCode code;
    size_t image_index;
    size_t section_offset;
} ScoopStackMapError;

bool scoop_stackmap_build_index(const ScoopStackMapImage *images,
                                size_t image_count,
                                ScoopStackMapIndex *index,
                                ScoopStackMapError *error);
void scoop_stackmap_dispose_index(ScoopStackMapIndex *index);
const ScoopStackMapRecord *
scoop_stackmap_lookup(const ScoopStackMapIndex *index, uintptr_t return_pc);
const char *scoop_stackmap_error_message(ScoopStackMapErrorCode code);

#endif /* SCOOP_GC_STACKMAP_H */
